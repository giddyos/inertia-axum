use super::{AssetBody, AssetRequest, AssetResponse, AssetSource};
use bytes::Bytes;
use http::{
    HeaderMap, HeaderValue, Method, StatusCode,
    header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
const REVALIDATE_CACHE: &str = "no-cache";

/// Framework-neutral source for files beneath one canonical directory.
#[derive(Clone, Debug)]
pub struct DirectoryAssetSource {
    root: Arc<PathBuf>,
}

impl DirectoryAssetSource {
    /// Creates a source rooted at an existing directory.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "asset root is not a directory",
            ));
        }
        Ok(Self {
            root: Arc::new(root),
        })
    }

    fn resolve(&self, path: &str) -> Option<PathBuf> {
        if path.is_empty() || path.contains(['\\', '\0']) {
            return None;
        }
        let relative = Path::new(path);
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return None;
        }
        let resolved = fs::canonicalize(self.root.join(relative)).ok()?;
        resolved.starts_with(self.root.as_ref()).then_some(resolved)
    }
}

impl AssetSource for DirectoryAssetSource {
    fn get(&self, request: AssetRequest<'_>) -> Option<AssetResponse> {
        if request.method != Method::GET && request.method != Method::HEAD {
            let mut headers = HeaderMap::new();
            headers.insert(ALLOW, HeaderValue::from_static("GET, HEAD"));
            return Some(AssetResponse {
                status: StatusCode::METHOD_NOT_ALLOWED,
                headers,
                body: AssetBody::Empty,
            });
        }

        let resolved = self.resolve(request.path)?;
        let metadata = fs::metadata(&resolved).ok()?;
        if !metadata.is_file() {
            return None;
        }

        let (etag, body, content_length) = if request.method == Method::HEAD {
            (
                hash_reader(File::open(&resolved).ok()?)?,
                AssetBody::Empty,
                metadata.len(),
            )
        } else {
            let bytes = fs::read(&resolved).ok()?;
            let etag = hash_bytes(&bytes);
            let length = u64::try_from(bytes.len()).ok()?;
            (etag, AssetBody::Bytes(Bytes::from(bytes)), length)
        };
        let cache = if is_content_addressed(request.path) {
            IMMUTABLE_CACHE
        } else {
            REVALIDATE_CACHE
        };
        let mut headers = HeaderMap::new();
        headers.insert(ETAG, HeaderValue::from_str(&etag).ok()?);
        headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));

        if etag_matches(request.headers, &etag) {
            return Some(AssetResponse {
                status: StatusCode::NOT_MODIFIED,
                headers,
                body: AssetBody::Empty,
            });
        }

        let content_type = mime_guess::from_path(request.path).first_or_octet_stream();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(content_type.as_ref()).ok()?,
        );
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&content_length.to_string()).ok()?,
        );
        Some(AssetResponse {
            status: StatusCode::OK,
            headers,
            body,
        })
    }
}

fn hash_reader(mut reader: impl io::Read) -> Option<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(format_etag(hasher.finalize()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format_etag(hasher.finalize())
}

fn format_etag(hash: impl AsRef<[u8]>) -> String {
    let mut value = String::with_capacity(66);
    value.push('"');
    for byte in hash.as_ref() {
        use std::fmt::Write as _;
        write!(value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value.push('"');
    value
}

fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate.trim_start_matches("W/") == etag)
}

fn is_content_addressed(path: &str) -> bool {
    let Some(file_name) = path.rsplit('/').next() else {
        return false;
    };
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    stem.split(['.', '-', '_']).skip(1).any(|segment| {
        segment.len() >= 8 && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "inertia-directory-assets-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("nested")).unwrap();
            fs::write(path.join("nested/app-12345678.js"), b"export default 1").unwrap();
            fs::write(path.join("plain.css"), b"body{}").unwrap();
            fs::write(path.join("remaining.txt"), b"ordinary").unwrap();
            Self(path)
        }

        fn source(&self) -> DirectoryAssetSource {
            DirectoryAssetSource::new(&self.0).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn request<'a>(method: &'a Method, path: &'a str, headers: &'a HeaderMap) -> AssetRequest<'a> {
        AssetRequest {
            method,
            path,
            headers,
        }
    }

    #[test]
    fn get_returns_bytes_metadata_etag_and_immutable_cache_policy() {
        let fixture = Fixture::new();
        let response = fixture
            .source()
            .get(request(
                &Method::GET,
                "nested/app-12345678.js",
                &HeaderMap::new(),
            ))
            .unwrap();
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.headers[CONTENT_TYPE], "text/javascript");
        assert_eq!(response.headers[CONTENT_LENGTH], "16");
        assert_eq!(response.headers[CACHE_CONTROL], IMMUTABLE_CACHE);
        assert!(response.headers[ETAG].to_str().unwrap().starts_with('"'));
        assert!(matches!(response.body, AssetBody::Bytes(bytes) if bytes == "export default 1"));
    }

    #[test]
    fn head_has_get_metadata_without_allocating_a_response_body() {
        let fixture = Fixture::new();
        let source = fixture.source();
        let get = source
            .get(request(&Method::GET, "plain.css", &HeaderMap::new()))
            .unwrap();
        let head = source
            .get(request(&Method::HEAD, "plain.css", &HeaderMap::new()))
            .unwrap();
        assert_eq!(head.status, StatusCode::OK);
        assert_eq!(head.headers[CONTENT_TYPE], "text/css");
        assert_eq!(head.headers[CONTENT_LENGTH], "6");
        assert_eq!(head.headers[ETAG], get.headers[ETAG]);
        assert_eq!(head.headers[CACHE_CONTROL], REVALIDATE_CACHE);
        assert!(matches!(head.body, AssetBody::Empty));
    }

    #[test]
    fn ordinary_long_names_are_not_mistaken_for_content_hashes() {
        let fixture = Fixture::new();
        let response = fixture
            .source()
            .get(request(&Method::GET, "remaining.txt", &HeaderMap::new()))
            .unwrap();
        assert_eq!(response.headers[CACHE_CONTROL], REVALIDATE_CACHE);
        assert!(!is_content_addressed("remaining.txt"));
        for path in ["app-C6R2N8QK.js", "app.30f2a8d9.js", "chunk_91a0f52c.css"] {
            assert!(is_content_addressed(path), "{path}");
        }
    }

    #[test]
    fn matching_etag_returns_empty_not_modified_response() {
        let fixture = Fixture::new();
        let source = fixture.source();
        let initial = source
            .get(request(&Method::GET, "plain.css", &HeaderMap::new()))
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(IF_NONE_MATCH, initial.headers[ETAG].clone());
        let response = source
            .get(request(&Method::GET, "plain.css", &headers))
            .unwrap();
        assert_eq!(response.status, StatusCode::NOT_MODIFIED);
        assert_eq!(response.headers[ETAG], initial.headers[ETAG]);
        assert!(matches!(response.body, AssetBody::Empty));
        assert!(!response.headers.contains_key(CONTENT_LENGTH));
    }

    #[test]
    fn missing_directories_and_traversal_are_not_exposed() {
        let fixture = Fixture::new();
        let source = fixture.source();
        for path in [
            "missing.js",
            "nested",
            "../secret",
            "nested/../../secret",
            "/absolute.js",
            r"nested\app-12345678.js",
        ] {
            assert!(
                source
                    .get(request(&Method::GET, path, &HeaderMap::new()))
                    .is_none(),
                "{path} must not resolve"
            );
        }
    }

    #[test]
    fn unsupported_methods_are_explicitly_rejected() {
        let fixture = Fixture::new();
        let response = fixture
            .source()
            .get(request(&Method::POST, "plain.css", &HeaderMap::new()))
            .unwrap();
        assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers[ALLOW], "GET, HEAD");
        assert!(matches!(response.body, AssetBody::Empty));
    }
}
