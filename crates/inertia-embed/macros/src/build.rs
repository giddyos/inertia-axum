use crate::input::EmbedInput;
use brotli::enc::{BrotliCompress, BrotliEncoderParams, backward_references::BrotliEncoderMode};
use proc_macro2::Span;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Read as _,
    path::{Path, PathBuf},
};
use syn::Error;
use walkdir::WalkDir;

const FORMAT_VERSION: &[u8] = b"inertia-embed-v1";

#[derive(Debug)]
pub(crate) struct BuiltFrontend {
    pub(crate) public_path: String,
    pub(crate) entry: String,
    pub(crate) version: String,
    pub(crate) tags: String,
    pub(crate) manifest: PathBuf,
    pub(crate) assets: Vec<BuiltAsset>,
}

#[derive(Debug)]
pub(crate) struct BuiltAsset {
    pub(crate) path: String,
    pub(crate) bytes: BuiltAssetBytes,
    pub(crate) storage: BuiltStorage,
    pub(crate) content_type: String,
    pub(crate) etag: String,
    pub(crate) immutable: bool,
}

#[derive(Debug)]
pub(crate) enum BuiltAssetBytes {
    File(PathBuf),
    Generated { bytes: Vec<u8>, source: PathBuf },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BuiltStorage {
    Identity,
    Brotli { uncompressed_len: usize },
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    file: String,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    assets: Vec<String>,
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default, rename = "dynamicImports")]
    dynamic_imports: Vec<String>,
}

#[derive(Debug)]
struct NormalEntry {
    file: String,
    css: Vec<String>,
    assets: Vec<String>,
    imports: Vec<String>,
}

#[derive(Default)]
struct Graph {
    files: Vec<String>,
    file_seen: BTreeSet<String>,
    css: BTreeSet<String>,
    referenced: BTreeSet<String>,
}

struct SelectedFile {
    logical: String,
    absolute: PathBuf,
}

pub(crate) fn build(input: &EmbedInput) -> syn::Result<BuiltFrontend> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| Error::new(Span::call_site(), "CARGO_MANIFEST_DIR is unavailable"))?;
    let root_label = input.root.value();
    let root_candidate = resolve_config_path(&manifest_dir, &root_label);
    let root = fs::canonicalize(&root_candidate).map_err(|error| {
        Error::new(
            input.root.span(),
            format!("embedded frontend root `{root_label}` is unavailable: {error}"),
        )
    })?;
    if !root.is_dir() {
        return Err(Error::new(
            input.root.span(),
            format!("embedded frontend root `{root_label}` is not a directory"),
        ));
    }

    let (manifest_candidate, manifest_label) = input.manifest.as_ref().map_or_else(
        || {
            (
                root.join(".vite/manifest.json"),
                format!("{root_label}/.vite/manifest.json"),
            )
        },
        |manifest| {
            let label = manifest.value();
            (resolve_config_path(&manifest_dir, &label), label)
        },
    );
    let manifest = fs::canonicalize(&manifest_candidate).map_err(|error| {
        Error::new(
            input
                .manifest
                .as_ref()
                .map_or_else(|| input.root.span(), syn::LitStr::span),
            format!("Vite manifest `{manifest_label}` is unavailable: {error}"),
        )
    })?;
    ensure_within_root(&root, &manifest, "Vite manifest", &manifest_label)?;
    let manifest_metadata = fs::metadata(&manifest).map_err(|error| {
        Error::new(
            Span::call_site(),
            format!("could not inspect Vite manifest `{manifest_label}`: {error}"),
        )
    })?;
    if manifest_metadata.len() > input.max_manifest_size {
        return Err(Error::new(
            Span::call_site(),
            format!(
                "Vite manifest `{manifest_label}` is {} bytes, exceeding the {} byte limit",
                manifest_metadata.len(),
                input.max_manifest_size
            ),
        ));
    }
    let source = fs::read_to_string(&manifest).map_err(|error| {
        Error::new(
            Span::call_site(),
            format!("could not read Vite manifest `{manifest_label}`: {error}"),
        )
    })?;
    let raw_manifest: BTreeMap<String, ManifestEntry> =
        serde_json::from_str(&source).map_err(|error| {
            Error::new(
                Span::call_site(),
                format!("Vite manifest `{manifest_label}` is invalid JSON: {error}"),
            )
        })?;
    let entries = normalize_manifest(raw_manifest)?;
    let entry = normalize_relative(&input.entry.value(), "entry")?;
    if !entries.contains_key(&entry) {
        return Err(Error::new(
            input.entry.span(),
            format!("Vite entry `{entry}` was not found in `{manifest_label}`"),
        ));
    }
    let public_path = normalize_public_path(&input.public_path.value(), input.public_path.span())?;
    let mut graph = Graph::default();
    resolve_graph(&entry, &entries, &mut BTreeSet::new(), &mut graph)?;

    let selected = collect_files(input, &root, &manifest)?;
    for required in &graph.referenced {
        if !selected.contains_key(required) {
            return Err(Error::new(
                Span::call_site(),
                format!(
                    "manifest-referenced emitted file `{required}` is missing or excluded by the hidden/source-map policy"
                ),
            ));
        }
    }

    let mut version_hasher = Sha256::new();
    version_hasher.update(FORMAT_VERSION);
    version_hasher.update([0]);
    version_hasher.update(public_path.as_bytes());
    version_hasher.update([0]);
    version_hasher.update(entry.as_bytes());
    version_hasher.update([0]);
    let mut assets = Vec::with_capacity(selected.len());
    for file in selected.values() {
        version_hasher.update(file.logical.as_bytes());
        version_hasher.update([0]);
        let (etag, content_type) = hash_asset(file, &mut version_hasher)?;
        version_hasher.update([0]);
        let immutable = is_content_addressed(&file.logical);
        let (bytes, storage) = if is_brotli_candidate(&file.logical) {
            compress_brotli(file, &content_type)?.map_or_else(
                || {
                    (
                        BuiltAssetBytes::File(file.absolute.clone()),
                        BuiltStorage::Identity,
                    )
                },
                |(bytes, uncompressed_len)| {
                    (
                        BuiltAssetBytes::Generated {
                            bytes,
                            source: file.absolute.clone(),
                        },
                        BuiltStorage::Brotli { uncompressed_len },
                    )
                },
            )
        } else {
            (
                BuiltAssetBytes::File(file.absolute.clone()),
                BuiltStorage::Identity,
            )
        };
        assets.push(BuiltAsset {
            path: encode_url_path(&file.logical),
            bytes,
            storage,
            content_type,
            etag,
            immutable,
        });
    }
    assets.sort_by(|left, right| left.path.cmp(&right.path));
    let version = format!("frontend-sha256-{}", hex(version_hasher.finalize()));
    let tags = build_tags(&public_path, &graph, &entries, &entry);
    Ok(BuiltFrontend {
        public_path,
        entry,
        version,
        tags,
        manifest,
        assets,
    })
}

fn resolve_config_path(base: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

fn ensure_within_root(root: &Path, candidate: &Path, kind: &str, label: &str) -> syn::Result<()> {
    if candidate.starts_with(root) {
        Ok(())
    } else {
        Err(Error::new(
            Span::call_site(),
            format!("{kind} `{label}` resolves outside the embedded frontend root"),
        ))
    }
}

fn normalize_manifest(
    raw: BTreeMap<String, ManifestEntry>,
) -> syn::Result<BTreeMap<String, NormalEntry>> {
    let mut entries = BTreeMap::new();
    let mut emitted = BTreeMap::<String, String>::new();
    for (raw_key, value) in raw {
        let key = normalize_relative(&raw_key, "manifest key")?;
        if entries.contains_key(&key) {
            return Err(Error::new(
                Span::call_site(),
                format!("duplicate normalized Vite manifest key `{key}`"),
            ));
        }
        let file = normalize_emitted_unique(&value.file, &mut emitted)?;
        let css = value
            .css
            .iter()
            .map(|path| normalize_emitted_unique(path, &mut emitted))
            .collect::<syn::Result<Vec<_>>>()?;
        let assets = value
            .assets
            .iter()
            .map(|path| normalize_emitted_unique(path, &mut emitted))
            .collect::<syn::Result<Vec<_>>>()?;
        let imports = value
            .imports
            .iter()
            .chain(&value.dynamic_imports)
            .map(|import| normalize_relative(import, "manifest import"))
            .collect::<syn::Result<Vec<_>>>()?;
        entries.insert(
            key,
            NormalEntry {
                file,
                css,
                assets,
                imports,
            },
        );
    }
    Ok(entries)
}

fn normalize_emitted_unique(raw: &str, seen: &mut BTreeMap<String, String>) -> syn::Result<String> {
    let normalized = normalize_relative(raw, "emitted path")?;
    if let Some(previous) = seen.get(&normalized) {
        if previous != raw {
            return Err(Error::new(
                Span::call_site(),
                format!(
                    "duplicate normalized emitted path `{normalized}` from `{previous}` and `{raw}`"
                ),
            ));
        }
    } else {
        seen.insert(normalized.clone(), raw.to_owned());
    }
    Ok(normalized)
}

fn normalize_relative(raw: &str, kind: &str) -> syn::Result<String> {
    if raw.contains(['\0', '\r', '\n']) {
        return Err(Error::new(
            Span::call_site(),
            format!("{kind} contains a forbidden NUL, CR, or LF character"),
        ));
    }
    let normalized = raw.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b':')
    {
        return Err(Error::new(
            Span::call_site(),
            format!("{kind} `{raw}` must be relative"),
        ));
    }
    let mut output = Vec::new();
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(Error::new(
                Span::call_site(),
                format!("{kind} `{raw}` contains an empty or traversal path segment"),
            ));
        }
        output.push(segment);
    }
    Ok(output.join("/"))
}

fn normalize_public_path(raw: &str, span: Span) -> syn::Result<String> {
    let normalized = if raw == "/" {
        "/"
    } else {
        raw.trim_end_matches('/')
    };
    if !normalized.starts_with('/')
        || normalized.is_empty()
        || normalized.contains([
            '\0', '\r', '\n', '\\', '"', '\'', '<', '>', '&', '{', '}', '?', '#',
        ])
        || !normalized.is_ascii()
        || (normalized != "/"
            && normalized.strip_prefix('/').is_none_or(|path| {
                path.split('/').any(|segment| {
                    segment.is_empty()
                        || segment == "."
                        || segment == ".."
                        || !segment.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric()
                                || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'%')
                        })
                })
            }))
    {
        return Err(Error::new(
            span,
            format!("unsafe embedded frontend public_path `{raw}`"),
        ));
    }
    Ok(normalized.to_owned())
}

fn resolve_graph(
    key: &str,
    entries: &BTreeMap<String, NormalEntry>,
    visited: &mut BTreeSet<String>,
    graph: &mut Graph,
) -> syn::Result<()> {
    if !visited.insert(key.to_owned()) {
        return Ok(());
    }
    let entry = entries.get(key).ok_or_else(|| {
        Error::new(
            Span::call_site(),
            format!("Vite manifest import `{key}` was not found"),
        )
    })?;
    if graph.file_seen.insert(entry.file.clone()) {
        graph.files.push(entry.file.clone());
    }
    graph.referenced.insert(entry.file.clone());
    graph.css.extend(entry.css.iter().cloned());
    graph.referenced.extend(entry.css.iter().cloned());
    graph.referenced.extend(entry.assets.iter().cloned());
    for import in &entry.imports {
        resolve_graph(import, entries, visited, graph)?;
    }
    Ok(())
}

fn collect_files(
    input: &EmbedInput,
    root: &Path,
    manifest: &Path,
) -> syn::Result<BTreeMap<String, SelectedFile>> {
    let mut selected = BTreeMap::new();
    let mut total = 0_u64;
    for result in WalkDir::new(root).follow_links(false) {
        let entry = result.map_err(|error| {
            Error::new(
                Span::call_site(),
                format!("could not walk embedded frontend output: {error}"),
            )
        })?;
        if entry.path() == manifest || entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(root).map_err(|_| {
            Error::new(
                Span::call_site(),
                "embedded output entry escaped its canonical root",
            )
        })?;
        let relative = relative.to_str().ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "embedded output contains a non-UTF-8 filename",
            )
        })?;
        let logical = normalize_relative(relative, "output path")?;
        if entry.file_type().is_symlink() {
            let target = fs::canonicalize(entry.path()).map_err(|error| {
                Error::new(
                    Span::call_site(),
                    format!("could not resolve output symlink `{logical}`: {error}"),
                )
            })?;
            ensure_within_root(root, &target, "output symlink", &logical)?;
            return Err(Error::new(
                Span::call_site(),
                format!("output symlink `{logical}` is unsupported; emit a regular file"),
            ));
        }
        if !entry.file_type().is_file()
            || (!input.include_hidden && is_hidden(&logical))
            || (!input.include_source_maps
                && Path::new(&logical)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("map")))
        {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| {
            Error::new(
                Span::call_site(),
                format!("could not inspect emitted file `{logical}`: {error}"),
            )
        })?;
        if metadata.len() > input.max_asset_size {
            return Err(Error::new(
                Span::call_site(),
                format!(
                    "emitted file `{logical}` is {} bytes, exceeding the {} byte individual-asset limit",
                    metadata.len(),
                    input.max_asset_size
                ),
            ));
        }
        total = total.checked_add(metadata.len()).ok_or_else(|| {
            Error::new(Span::call_site(), "embedded frontend total size overflowed")
        })?;
        if input.max_total_size != 0 && total > input.max_total_size {
            return Err(Error::new(
                Span::call_site(),
                format!(
                    "embedded frontend is {total} bytes, exceeding the {} byte total-size limit",
                    input.max_total_size
                ),
            ));
        }
        if u64::try_from(selected.len()).unwrap_or(u64::MAX) >= input.max_files {
            return Err(Error::new(
                Span::call_site(),
                format!(
                    "embedded frontend exceeds the {} file limit",
                    input.max_files
                ),
            ));
        }
        let absolute = fs::canonicalize(entry.path()).map_err(|error| {
            Error::new(
                Span::call_site(),
                format!("could not canonicalize emitted file `{logical}`: {error}"),
            )
        })?;
        ensure_within_root(root, &absolute, "emitted file", &logical)?;
        if selected
            .insert(logical.clone(), SelectedFile { logical, absolute })
            .is_some()
        {
            return Err(Error::new(
                Span::call_site(),
                format!("duplicate normalized output path `{relative}`"),
            ));
        }
    }
    Ok(selected)
}

fn is_hidden(path: &str) -> bool {
    path.split('/')
        .any(|segment| segment.starts_with('.') && segment != "." && segment != "..")
}

fn hash_asset(file: &SelectedFile, version: &mut Sha256) -> syn::Result<(String, String)> {
    let mut input = fs::File::open(&file.absolute).map_err(|error| {
        Error::new(
            Span::call_site(),
            format!("could not open emitted file `{}`: {error}", file.logical),
        )
    })?;
    let mut asset = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = input.read(&mut buffer).map_err(|error| {
            Error::new(
                Span::call_site(),
                format!("could not read emitted file `{}`: {error}", file.logical),
            )
        })?;
        if read == 0 {
            break;
        }
        asset.update(&buffer[..read]);
        version.update(&buffer[..read]);
    }
    let etag = format!("\"sha256-{}\"", hex(asset.finalize()));
    let mime = mime_guess::from_path(&file.logical).first_or_octet_stream();
    let essence = mime.essence_str();
    let content_type = if essence.starts_with("text/")
        || matches!(
            essence,
            "application/javascript" | "application/json" | "application/xml" | "image/svg+xml"
        ) {
        format!("{essence}; charset=utf-8")
    } else {
        essence.to_owned()
    };
    if content_type.contains(['\r', '\n']) {
        return Err(Error::new(
            Span::call_site(),
            format!("invalid content type generated for `{}`", file.logical),
        ));
    }
    Ok((etag, content_type))
}

fn is_brotli_candidate(logical: &str) -> bool {
    let extension = Path::new(logical)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    ![
        "7z", "avif", "br", "bz2", "gif", "gz", "jpeg", "jpg", "mp3", "mp4", "ogg", "pdf", "png",
        "rar", "webm", "webp", "woff", "woff2", "xz", "zip", "zst",
    ]
    .iter()
    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
}

fn compress_brotli(
    file: &SelectedFile,
    content_type: &str,
) -> syn::Result<Option<(Vec<u8>, usize)>> {
    let mut input = fs::File::open(&file.absolute).map_err(|error| {
        Error::new(
            Span::call_site(),
            format!(
                "could not open emitted file `{}` for Brotli compression: {error}",
                file.logical
            ),
        )
    })?;
    let source_size = input
        .metadata()
        .map_err(|error| {
            Error::new(
                Span::call_site(),
                format!(
                    "could not inspect emitted file `{}` for Brotli compression: {error}",
                    file.logical
                ),
            )
        })?
        .len();
    let mut output = Vec::new();
    let mut params = BrotliEncoderParams {
        quality: 11,
        lgwin: 24,
        size_hint: usize::try_from(source_size).unwrap_or(usize::MAX),
        ..BrotliEncoderParams::default()
    };
    if is_textual_content(content_type) {
        params.mode = BrotliEncoderMode::BROTLI_MODE_TEXT;
    }
    BrotliCompress(&mut input, &mut output, &params).map_err(|error| {
        Error::new(
            Span::call_site(),
            format!(
                "could not Brotli-compress emitted file `{}`: {error}",
                file.logical
            ),
        )
    })?;
    let uncompressed_len = usize::try_from(source_size).map_err(|_| {
        Error::new(
            Span::call_site(),
            format!(
                "emitted file `{}` cannot fit in this target's address space",
                file.logical
            ),
        )
    })?;
    Ok((output.len() < uncompressed_len).then_some((output, uncompressed_len)))
}

fn is_textual_content(content_type: &str) -> bool {
    let essence = content_type.split(';').next().unwrap_or(content_type);
    essence.starts_with("text/")
        || essence.ends_with("+json")
        || essence.ends_with("+xml")
        || matches!(
            essence,
            "application/javascript" | "application/json" | "application/xml" | "image/svg+xml"
        )
}

fn build_tags(
    public_path: &str,
    graph: &Graph,
    entries: &BTreeMap<String, NormalEntry>,
    entry: &str,
) -> String {
    let prefix = if public_path == "/" { "" } else { public_path };
    let mut tags = String::new();
    for path in &graph.css {
        tags.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"{prefix}/{}\">",
            encode_url_path(path)
        ));
    }
    let entry_file = &entries[entry].file;
    for path in graph.files.iter().filter(|path| *path != entry_file) {
        tags.push_str(&format!(
            "<link rel=\"modulepreload\" href=\"{prefix}/{}\">",
            encode_url_path(path)
        ));
    }
    tags.push_str(&format!(
        "<script type=\"module\" src=\"{prefix}/{}\"></script>",
        encode_url_path(entry_file)
    ));
    tags
}

pub(crate) fn encode_url_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn is_content_addressed(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    stem.split(['.', '-', '_']).skip(1).any(|segment| {
        segment.len() >= 8 && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn hex(hash: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(hash.as_ref().len() * 2);
    for byte in hash.as_ref() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::is_content_addressed;

    #[test]
    fn content_hash_requires_a_distinct_hash_like_segment() {
        for path in [
            "assets/app-C6R2N8QK.js",
            "assets/app.30f2a8d9.js",
            "assets/chunk_91a0f52c.css",
        ] {
            assert!(is_content_addressed(path), "{path}");
        }
        for path in ["remaining.txt", "assets/stylesheet.css", "images/pixel.bin"] {
            assert!(!is_content_addressed(path), "{path}");
        }
    }
}
