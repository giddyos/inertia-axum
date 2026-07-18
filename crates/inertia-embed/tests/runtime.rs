//! Runtime and generated metadata coverage.

use http::{
    HeaderMap, Method, StatusCode,
    header::{
        ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, ETAG,
        IF_NONE_MATCH, VARY,
    },
};
use inertia_core::{AssetBody, AssetProvider as _, AssetRequest, AssetSource as _};
use inertia_embed::{EmbeddedAsset, EmbeddedFrontend, EmbeddedStorage, embed_frontend};
use sha2::{Digest as _, Sha256};

static FRONTEND: EmbeddedFrontend = embed_frontend! {
    root: "tests/fixtures/valid/dist",
    entry: "src/main.ts",
};

static FRONTEND_COPY: EmbeddedFrontend = embed_frontend! {
    root: "tests/fixtures/valid/dist",
    manifest: "tests/fixtures/valid/dist/.vite/manifest.json",
    entry: "src/main.ts",
    public_path: "/build",
    cache: "auto",
};

static COMPLETE_FRONTEND: EmbeddedFrontend = embed_frontend! {
    root: "tests/fixtures/valid/dist",
    entry: "src/main.ts",
    public_path: "/assets",
    include_source_maps: true,
    include_hidden: true,
    max_total_size: 0,
};

static ENCODED_ASSETS: &[EmbeddedAsset] = &[EmbeddedAsset {
    path: "assets/precompressed.js",
    bytes: b"encoded",
    storage: EmbeddedStorage::Identity,
    content_type: "text/javascript; charset=utf-8",
    etag: "\"encoded-etag\"",
    immutable: false,
    encoding: Some("br"),
}];

static ENCODED_FRONTEND: EmbeddedFrontend = EmbeddedFrontend::new(
    "/build",
    "src/main.ts",
    "encoded-version",
    "",
    ENCODED_ASSETS,
);

static CORRUPT_ASSETS: &[EmbeddedAsset] = &[EmbeddedAsset {
    path: "assets/corrupt.txt",
    bytes: b"not-brotli",
    storage: EmbeddedStorage::Brotli {
        uncompressed_len: 128,
    },
    content_type: "text/plain; charset=utf-8",
    etag: "\"corrupt-etag\"",
    immutable: false,
    encoding: None,
}];

static CORRUPT_FRONTEND: EmbeddedFrontend = EmbeddedFrontend::new(
    "/build",
    "src/main.ts",
    "corrupt-version",
    "",
    CORRUPT_ASSETS,
);

fn request<'a>(method: &'a Method, path: &'a str, headers: &'a HeaderMap) -> AssetRequest<'a> {
    AssetRequest {
        method,
        path,
        headers,
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn body_bytes(body: &AssetBody) -> &[u8] {
    match body {
        AssetBody::Empty => &[],
        AssetBody::Static(bytes) => bytes,
        AssetBody::Bytes(bytes) => bytes.as_ref(),
    }
}

#[test]
fn generated_metadata_is_sorted_complete_and_deterministic() {
    fn assert_clone_copy<T: Clone + Copy>() {}

    assert_clone_copy::<EmbeddedFrontend>();
    assert_eq!(FRONTEND.public_path, "/build");
    assert_eq!(FRONTEND.entry, "src/main.ts");
    assert!(FRONTEND.version.starts_with("frontend-sha256-"));
    assert_eq!(FRONTEND.version, FRONTEND_COPY.version);
    assert_eq!(FRONTEND.tags, FRONTEND_COPY.tags);
    assert!(
        FRONTEND
            .assets
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
    );

    for path in [
        "assets/main-C6R2N8QK.js",
        "assets/shared.91a0f52c.js",
        "assets/nested-1234abcd.js",
        "assets/prebuilt.js.br",
        "assets/dynamic-abcdef12.js",
        "assets/main-30f2a8d9.css",
        "assets/shared.css",
        "assets/theme%26print.css",
        "assets/windows-ABCDEF12.js",
        "assets/file%20name.txt",
        "assets/remaining.txt",
        "assets/repetitive-data.txt",
        "images/pixel.bin",
    ] {
        assert!(FRONTEND.find(path).is_some(), "{path} must be embedded");
    }
    assert!(FRONTEND.find("assets/source.js.map").is_none());
    assert!(FRONTEND.find(".hidden.txt").is_none());
    assert!(COMPLETE_FRONTEND.find("assets/source.js.map").is_some());
    assert!(COMPLETE_FRONTEND.find(".hidden.txt").is_some());
    assert!(std::str::from_utf8(FRONTEND.find("images/pixel.bin").unwrap().bytes).is_err());
}

#[test]
fn deployment_version_and_etags_match_the_documented_byte_stream() {
    let paths = [
        "assets/dynamic-abcdef12.js",
        "assets/file name.txt",
        "assets/main-30f2a8d9.css",
        "assets/main-C6R2N8QK.js",
        "assets/nested-1234abcd.js",
        "assets/prebuilt.js.br",
        "assets/remaining.txt",
        "assets/repetitive-data.txt",
        "assets/shared.91a0f52c.js",
        "assets/shared.css",
        "assets/theme&print.css",
        "assets/windows-ABCDEF12.js",
        "images/pixel.bin",
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/dist");
    let mut deployment = Sha256::new();
    for field in ["inertia-embed-v1", FRONTEND.public_path, FRONTEND.entry] {
        deployment.update(field.as_bytes());
        deployment.update([0]);
    }
    for path in paths {
        let bytes = std::fs::read(root.join(path)).unwrap();
        deployment.update(path.as_bytes());
        deployment.update([0]);
        deployment.update(&bytes);
        deployment.update([0]);
    }
    assert_eq!(
        FRONTEND.version,
        format!("frontend-sha256-{}", hex(&deployment.finalize()))
    );

    let asset = FRONTEND.find("assets/main-C6R2N8QK.js").unwrap();
    let emitted = std::fs::read(root.join("assets/main-C6R2N8QK.js")).unwrap();
    assert_eq!(
        asset.etag,
        format!("\"sha256-{}\"", hex(&Sha256::digest(emitted)))
    );

    let prebuilt = FRONTEND.find("assets/prebuilt.js.br").unwrap();
    assert_eq!(
        prebuilt.bytes,
        include_bytes!("fixtures/valid/dist/assets/prebuilt.js.br")
    );
    assert_eq!(prebuilt.storage, EmbeddedStorage::Identity);
    assert_eq!(prebuilt.encoding, None);
    assert!(!prebuilt.immutable);
    assert!(!FRONTEND.find("assets/remaining.txt").unwrap().immutable);
}

#[test]
fn executable_storage_is_aggressively_compressed_but_http_bytes_are_original() {
    let path = "assets/repetitive-data.txt";
    let emitted = include_bytes!("fixtures/valid/dist/assets/repetitive-data.txt");
    let asset = FRONTEND.find(path).unwrap();
    assert!(asset.is_storage_compressed());
    assert!(asset.bytes.len() < emitted.len());
    assert_eq!(asset.uncompressed_len(), emitted.len());

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_ENCODING, "br, gzip".parse().unwrap());
    let first = (&FRONTEND)
        .get(request(&Method::GET, path, &headers))
        .unwrap();
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.headers[CONTENT_LENGTH], emitted.len().to_string());
    assert!(!first.headers.contains_key(CONTENT_ENCODING));
    assert!(!first.headers.contains_key(VARY));
    assert_eq!(body_bytes(&first.body), emitted);

    let second = (&FRONTEND)
        .get(request(&Method::GET, path, &HeaderMap::new()))
        .unwrap();
    assert_eq!(body_bytes(&second.body), emitted);
}

#[test]
fn head_and_not_modified_do_not_expand_executable_storage() {
    let source = &CORRUPT_FRONTEND;
    let head = source
        .get(request(
            &Method::HEAD,
            "assets/corrupt.txt",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(head.headers[CONTENT_LENGTH], "128");

    let mut conditional = HeaderMap::new();
    conditional.insert(IF_NONE_MATCH, CORRUPT_ASSETS[0].etag.parse().unwrap());
    let not_modified = source
        .get(request(&Method::GET, "assets/corrupt.txt", &conditional))
        .unwrap();
    assert_eq!(not_modified.status, StatusCode::NOT_MODIFIED);

    let get = source
        .get(request(
            &Method::GET,
            "assets/corrupt.txt",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(get.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn explicit_content_encoding_preserves_vary_on_get_head_and_304() {
    let source = &ENCODED_FRONTEND;
    let get = source
        .get(request(
            &Method::GET,
            "assets/precompressed.js",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(get.headers[CONTENT_ENCODING], "br");
    assert_eq!(get.headers[VARY], "Accept-Encoding");
    assert!(matches!(get.body, AssetBody::Static(b"encoded")));

    let head = source
        .get(request(
            &Method::HEAD,
            "assets/precompressed.js",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(head.headers[CONTENT_ENCODING], "br");
    assert_eq!(head.headers[VARY], "Accept-Encoding");
    assert!(matches!(head.body, AssetBody::Empty));

    let mut conditional = HeaderMap::new();
    conditional.insert(IF_NONE_MATCH, get.headers[ETAG].clone());
    let not_modified = source
        .get(request(
            &Method::GET,
            "assets/precompressed.js",
            &conditional,
        ))
        .unwrap();
    assert_eq!(not_modified.status, StatusCode::NOT_MODIFIED);
    assert_eq!(not_modified.headers[CONTENT_ENCODING], "br");
    assert_eq!(not_modified.headers[VARY], "Accept-Encoding");
    assert!(matches!(not_modified.body, AssetBody::Empty));
}

#[test]
fn manifest_graph_tags_are_ordered_encoded_and_escaped() {
    let tags = FRONTEND.tags;
    let main_css = tags.find("/build/assets/main-30f2a8d9.css").unwrap();
    let shared_css = tags.find("/build/assets/shared.css").unwrap();
    let shared_js = tags.find("/build/assets/shared.91a0f52c.js").unwrap();
    let nested_js = tags.find("/build/assets/nested-1234abcd.js").unwrap();
    let dynamic_js = tags.find("/build/assets/dynamic-abcdef12.js").unwrap();
    let entry_js = tags.find("/build/assets/main-C6R2N8QK.js").unwrap();
    assert!(main_css < shared_css);
    assert!(shared_css < shared_js);
    assert!(shared_js < nested_js);
    assert!(nested_js < dynamic_js);
    assert!(dynamic_js < entry_js);
    assert!(tags.contains("/build/assets/theme%26print.css"));
    assert!(!tags.contains("theme&print.css"));
    assert!(COMPLETE_FRONTEND.tags.contains("/assets/"));
}

#[test]
fn exact_lookup_rejects_traversal_and_does_not_decode_paths() {
    assert!(FRONTEND.find("/assets/main-C6R2N8QK.js").is_some());
    assert!(FRONTEND.find("assets/file%20name.txt?download=1").is_some());
    for path in [
        "",
        "/",
        "../assets/main-C6R2N8QK.js",
        "assets/../main-C6R2N8QK.js",
        "assets\\main-C6R2N8QK.js",
        "assets/%2e%2e/main-C6R2N8QK.js",
        "assets",
    ] {
        assert!(FRONTEND.find(path).is_none(), "{path} must be rejected");
    }
}

#[test]
fn source_serves_get_head_etag_cache_and_method_responses() {
    let source = &FRONTEND;
    let get = source
        .get(request(
            &Method::GET,
            "assets/main-C6R2N8QK.js",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(get.status, StatusCode::OK);
    assert_eq!(get.headers[CONTENT_TYPE], "text/javascript; charset=utf-8");
    assert_eq!(
        get.headers[CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(
        get.headers[CONTENT_LENGTH],
        FRONTEND
            .find("assets/main-C6R2N8QK.js")
            .unwrap()
            .uncompressed_len()
            .to_string()
    );
    assert_eq!(
        body_bytes(&get.body),
        include_bytes!("fixtures/valid/dist/assets/main-C6R2N8QK.js")
    );

    let head = source
        .get(request(
            &Method::HEAD,
            "assets/main-C6R2N8QK.js",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(head.status, StatusCode::OK);
    assert_eq!(head.headers[ETAG], get.headers[ETAG]);
    assert!(matches!(head.body, AssetBody::Empty));

    let mut conditional = HeaderMap::new();
    conditional.insert(IF_NONE_MATCH, get.headers[ETAG].clone());
    let not_modified = source
        .get(request(
            &Method::GET,
            "assets/main-C6R2N8QK.js",
            &conditional,
        ))
        .unwrap();
    assert_eq!(not_modified.status, StatusCode::NOT_MODIFIED);
    assert!(!not_modified.headers.contains_key(CONTENT_LENGTH));
    assert!(matches!(not_modified.body, AssetBody::Empty));

    let method = source
        .get(request(
            &Method::POST,
            "assets/main-C6R2N8QK.js",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(
        source
            .version()
            .header_value()
            .starts_with("frontend-sha256-")
    );
    assert_eq!(source.public_path(), Some("/build"));
}

#[test]
fn unhashed_files_use_revalidation_cache_policy() {
    let response = (&FRONTEND)
        .get(request(
            &Method::GET,
            "assets/shared.css",
            &HeaderMap::new(),
        ))
        .unwrap();
    assert_eq!(
        response.headers[CACHE_CONTROL],
        "public, max-age=0, must-revalidate"
    );
}
