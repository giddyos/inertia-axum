//! Asset providers and convention-based Vite startup configuration.

use crate::root::AssetTags;
#[cfg(feature = "vite")]
use http::Uri;
#[cfg(feature = "vite")]
use serde::Deserialize;
use serde::Serialize;
use serde_json::Number;
#[cfg(feature = "vite")]
use sha2::{Digest, Sha256};
use std::{borrow::Cow, error::Error, fmt, sync::Arc};
#[cfg(feature = "vite")]
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

/// A scalar Inertia asset version retaining its JSON number-or-string form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum AssetVersion {
    /// String deployment identifier.
    String(Arc<str>),
    /// Numeric deployment identifier.
    Number(Number),
}

impl AssetVersion {
    /// Returns the normalized value used in the `X-Inertia-Version` header.
    pub fn header_value(&self) -> Cow<'_, str> {
        match self {
            Self::String(value) => Cow::Borrowed(value),
            Self::Number(value) => Cow::Owned(value.to_string()),
        }
    }
}

impl From<String> for AssetVersion {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}
impl From<&str> for AssetVersion {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}
impl From<Arc<str>> for AssetVersion {
    fn from(value: Arc<str>) -> Self {
        Self::String(value)
    }
}
impl From<u64> for AssetVersion {
    fn from(value: u64) -> Self {
        Self::Number(value.into())
    }
}

/// Request information available while rendering asset tags.
#[derive(Clone, Copy, Debug, Default)]
pub struct AssetContext<'a> {
    nonce: Option<&'a str>,
}

impl<'a> AssetContext<'a> {
    /// Creates a context with an optional CSP nonce.
    pub fn new(nonce: Option<&'a str>) -> Self {
        Self { nonce }
    }
    /// Returns the optional CSP nonce.
    pub fn nonce(&self) -> Option<&str> {
        self.nonce
    }
}

/// Asset provider failure.
pub type AssetError = Box<dyn Error + Send + Sync>;
/// Advanced interface for non-Vite asset pipelines.
pub trait AssetProvider: Clone + Send + Sync + 'static {
    /// Returns the scalar deployment version.
    fn version(&self) -> &AssetVersion;
    /// Renders safe script/style markup.
    fn render_tags(&self, context: AssetContext<'_>) -> Result<AssetTags, AssetError>;
}

pub(crate) trait ErasedAssetProvider: Send + Sync {
    fn build_runtime(&self, public_path: &str) -> Result<AssetRuntime, ConfigError>;
}

impl<P: AssetProvider> ErasedAssetProvider for P {
    fn build_runtime(&self, _public_path: &str) -> Result<AssetRuntime, ConfigError> {
        let tags = self.render_tags(AssetContext::default()).map_err(|error| {
            ConfigError::new(format!(
                "inertia-core asset configuration error\n\nCould not render asset tags: {error}"
            ))
        })?;
        let version = self.version().clone();
        let header_version = Arc::from(version.header_value().into_owned());
        Ok(AssetRuntime {
            version: Some(version),
            header_version: Some(header_version),
            tags,
            #[cfg(feature = "vite")]
            filesystem_mount: None,
            #[cfg(all(feature = "vite", feature = "ssr"))]
            vite_dev_server: None,
        })
    }
}

#[derive(Clone)]
pub(crate) struct AssetRuntime {
    pub(crate) version: Option<AssetVersion>,
    pub(crate) header_version: Option<Arc<str>>,
    pub(crate) tags: AssetTags,
    #[cfg(feature = "vite")]
    pub(crate) filesystem_mount: Option<(String, PathBuf)>,
    #[cfg(all(feature = "vite", feature = "ssr"))]
    pub(crate) vite_dev_server: Option<Arc<str>>,
}

impl Default for AssetRuntime {
    fn default() -> Self {
        Self {
            version: None,
            header_version: None,
            tags: AssetTags::empty(),
            #[cfg(feature = "vite")]
            filesystem_mount: None,
            #[cfg(all(feature = "vite", feature = "ssr"))]
            vite_dev_server: None,
        }
    }
}

/// Actionable application configuration failure.
#[derive(Debug)]
pub struct ConfigError(String);

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ConfigError {}

#[derive(Clone, Debug)]
#[cfg(feature = "vite")]
pub(crate) struct ViteConfig {
    pub(crate) root: PathBuf,
    pub(crate) entry: PathBuf,
    pub(crate) build_dir: PathBuf,
    pub(crate) public_path: String,
    pub(crate) dev_server: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg(feature = "vite")]
struct ManifestEntry {
    file: String,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    imports: Vec<String>,
}

#[cfg(feature = "vite")]
impl ViteConfig {
    pub(crate) fn build(self) -> Result<AssetRuntime, ConfigError> {
        if let Some(url) = self
            .dev_server
            .clone()
            .or_else(|| std::env::var("VITE_DEV_SERVER_URL").ok())
        {
            return self.dev_runtime(&url);
        }
        self.production_runtime()
    }

    fn dev_runtime(&self, url: &str) -> Result<AssetRuntime, ConfigError> {
        let uri: Uri = url.parse().map_err(|error| ConfigError::new(format!(
            "inertia-core Vite configuration error\n\nVITE_DEV_SERVER_URL \"{url}\" is malformed: {error}"
        )))?;
        if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.authority().is_none() {
            return Err(ConfigError::new(format!(
                "inertia-core Vite configuration error\n\nVITE_DEV_SERVER_URL \"{url}\" must be an absolute http:// or https:// URL"
            )));
        }
        let base = escape_attribute(url.trim_end_matches('/'));
        let entry = self
            .entry
            .to_string_lossy()
            .trim_start_matches('/')
            .to_owned();
        let entry = escape_attribute(&entry);
        let tags = AssetTags::new(format!(
            "<script type=\"module\" src=\"{base}/@vite/client\"></script><script type=\"module\" src=\"{base}/{entry}\"></script>"
        ));
        Ok(AssetRuntime {
            version: None,
            header_version: None,
            tags,
            filesystem_mount: None,
            #[cfg(feature = "ssr")]
            vite_dev_server: Some(Arc::from(url.trim_end_matches('/'))),
        })
    }

    fn production_runtime(&self) -> Result<AssetRuntime, ConfigError> {
        let build_dir = if self.build_dir.is_absolute() {
            self.build_dir.clone()
        } else {
            self.root.join(&self.build_dir)
        };
        let manifest_path = build_dir.join(".vite/manifest.json");
        let source = fs::read_to_string(&manifest_path).map_err(|error| ConfigError::new(format!(
            "inertia-core Vite configuration error\n\nCould not read manifest at {}: {error}\n\nRun the Vite production build or set VITE_DEV_SERVER_URL for development.", manifest_path.display()
        )))?;
        let manifest: BTreeMap<String, ManifestEntry> = serde_json::from_str(&source).map_err(|error| ConfigError::new(format!(
            "inertia-core Vite configuration error\n\nManifest {} is not valid JSON: {error}", manifest_path.display()
        )))?;
        let key = self.entry.to_string_lossy().replace('\\', "/");
        if !manifest.contains_key(&key) {
            let available = manifest
                .keys()
                .map(|key| format!("  - {key}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(ConfigError::new(format!(
                "inertia-core Vite configuration error\n\nEntry \"{key}\" was not found in:\n{}\n\nAvailable entries:\n{available}",
                manifest_path.display()
            )));
        }
        let mut visited = BTreeSet::new();
        let mut files = Vec::new();
        let mut css = BTreeSet::new();
        resolve_manifest(&key, &manifest, &mut visited, &mut files, &mut css)?;
        let entry_file = &manifest[&key].file;
        let prefix = self.public_path.trim_end_matches('/');
        if !prefix.starts_with('/') || prefix.contains(['"', '<', '>', '&']) {
            return Err(ConfigError::new(format!(
                "inertia-core Vite configuration error\n\nPublic path \"{}\" must be an absolute, HTML-safe URL path",
                self.public_path
            )));
        }
        let mut tags = String::new();
        for path in &css {
            let path = escape_attribute(path);
            tags.push_str(&format!(
                "<link rel=\"stylesheet\" href=\"{prefix}/{path}\">"
            ));
        }
        for path in files.iter().filter(|path| path.as_str() != entry_file) {
            let path = escape_attribute(path);
            tags.push_str(&format!(
                "<link rel=\"modulepreload\" href=\"{prefix}/{path}\">"
            ));
        }
        let entry_file = escape_attribute(entry_file);
        tags.push_str(&format!(
            "<script type=\"module\" src=\"{prefix}/{entry_file}\"></script>"
        ));
        let mut hasher = Sha256::new();
        for path in files.iter().chain(css.iter()) {
            hasher.update(path.as_bytes());
            hasher.update([0]);
        }
        let version = hasher
            .finalize()
            .iter()
            .flat_map(|byte| {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]]
            })
            .map(char::from)
            .collect::<String>();
        let header_version: Arc<str> = Arc::from(version.as_str());
        Ok(AssetRuntime {
            version: Some(AssetVersion::from(version)),
            header_version: Some(header_version),
            tags: AssetTags::new(tags),
            filesystem_mount: Some((self.public_path.clone(), build_dir)),
            #[cfg(feature = "ssr")]
            vite_dev_server: None,
        })
    }
}

#[cfg(feature = "vite")]
fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(feature = "vite")]
fn resolve_manifest(
    key: &str,
    manifest: &BTreeMap<String, ManifestEntry>,
    visited: &mut BTreeSet<String>,
    files: &mut Vec<String>,
    css: &mut BTreeSet<String>,
) -> Result<(), ConfigError> {
    if !visited.insert(key.to_owned()) {
        return Ok(());
    }
    let entry = manifest.get(key).ok_or_else(|| {
        ConfigError::new(format!(
            "inertia-core Vite configuration error\n\nManifest import \"{key}\" was not found"
        ))
    })?;
    files.push(entry.file.clone());
    css.extend(entry.css.iter().cloned());
    for import in &entry.imports {
        resolve_manifest(import, manifest, visited, files, css)?;
    }
    Ok(())
}
