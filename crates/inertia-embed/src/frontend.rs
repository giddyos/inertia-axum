use bytes::Bytes;
use inertia_core::{
    AssetContext, AssetError, AssetProvider, AssetRequest, AssetResponse, AssetSource, AssetTags,
    AssetVersion,
};
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, OnceLock, RwLock},
};

/// Storage representation used for bytes compiled into the executable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddedStorage {
    /// Bytes are stored exactly as emitted.
    Identity,
    /// Bytes are stored with Brotli and expanded before entering an HTTP adapter.
    Brotli {
        /// Exact length after decompression.
        uncompressed_len: usize,
    },
}

/// One emitted frontend asset compiled into the application binary.
#[derive(Clone, Copy, Debug)]
pub struct EmbeddedAsset {
    /// Percent-encoded, public-path-relative lookup path.
    pub path: &'static str,
    /// Static storage bytes compiled into the executable.
    ///
    /// Inspect [`EmbeddedAsset::storage`] before treating these as the emitted
    /// file bytes.
    pub bytes: &'static [u8],
    /// Compression used only for executable storage.
    pub storage: EmbeddedStorage,
    /// Valid response `Content-Type`.
    pub content_type: &'static str,
    /// Quoted compile-time SHA-256 entity tag.
    pub etag: &'static str,
    /// Whether the filename is conservatively recognized as content-addressed.
    pub immutable: bool,
    /// Optional HTTP content encoding for a directly addressed precompressed file.
    ///
    /// Compile-time executable storage compression does not set this field.
    pub encoding: Option<&'static str>,
}

impl EmbeddedAsset {
    /// Returns the exact emitted length after storage decompression.
    pub const fn uncompressed_len(&self) -> usize {
        match self.storage {
            EmbeddedStorage::Identity => self.bytes.len(),
            EmbeddedStorage::Brotli { uncompressed_len } => uncompressed_len,
        }
    }

    /// Returns whether the executable stores this asset with Brotli.
    pub const fn is_storage_compressed(&self) -> bool {
        matches!(self.storage, EmbeddedStorage::Brotli { .. })
    }
}

type CacheKey = (usize, usize, usize);
type DecompressionCache = RwLock<BTreeMap<CacheKey, Bytes>>;
static DECOMPRESSED: OnceLock<DecompressionCache> = OnceLock::new();

struct ExactSizeBuffer {
    bytes: Vec<u8>,
    expected: usize,
}

impl ExactSizeBuffer {
    fn new(expected: usize) -> Result<Self, DecompressionError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(expected)
            .map_err(|_| DecompressionError)?;
        Ok(Self { bytes, expected })
    }
}

impl io::Write for ExactSizeBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.expected.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "embedded asset expanded beyond its declared length",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A complete production frontend compiled into the application binary.
#[derive(Clone, Copy, Debug)]
pub struct EmbeddedFrontend {
    /// URL prefix under which adapters mount the asset source.
    pub public_path: &'static str,
    /// Vite manifest entry used to generate tags.
    pub entry: &'static str,
    /// Deterministic deployment version.
    pub version: &'static str,
    /// Trusted deterministic stylesheet, preload, and script markup.
    pub tags: &'static str,
    /// Sorted static asset table.
    pub assets: &'static [EmbeddedAsset],
}

impl EmbeddedFrontend {
    /// Creates a compile-time frontend value.
    pub const fn new(
        public_path: &'static str,
        entry: &'static str,
        version: &'static str,
        tags: &'static str,
        assets: &'static [EmbeddedAsset],
    ) -> Self {
        Self {
            public_path,
            entry,
            version,
            tags,
            assets,
        }
    }

    /// Finds an exact asset without allocation or path decoding.
    pub fn find(&self, request_path: &str) -> Option<&'static EmbeddedAsset> {
        let path = request_path
            .split_once('?')
            .map_or(request_path, |(path, _)| path);
        let path = path.strip_prefix('/').unwrap_or(path);
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return None;
        }
        self.assets
            .binary_search_by_key(&path, |asset| asset.path)
            .ok()
            .map(|index| &self.assets[index])
    }

    pub(crate) fn response_bytes(
        asset: &'static EmbeddedAsset,
    ) -> Result<Bytes, DecompressionError> {
        if asset.storage == EmbeddedStorage::Identity {
            return Ok(Bytes::from_static(asset.bytes));
        }
        let key = (
            asset.bytes.as_ptr() as usize,
            asset.bytes.len(),
            asset.uncompressed_len(),
        );
        let cache = DECOMPRESSED.get_or_init(|| RwLock::new(BTreeMap::new()));
        if let Some(bytes) = cache
            .read()
            .map_err(|_| DecompressionError)?
            .get(&key)
            .cloned()
        {
            return Ok(bytes);
        }

        let mut cache = cache.write().map_err(|_| DecompressionError)?;
        if let Some(bytes) = cache.get(&key).cloned() {
            return Ok(bytes);
        }
        let mut input = asset.bytes;
        let mut output = ExactSizeBuffer::new(asset.uncompressed_len())?;
        brotli_decompressor::BrotliDecompress(&mut input, &mut output)
            .map_err(|_| DecompressionError)?;
        if output.bytes.len() != asset.uncompressed_len() {
            return Err(DecompressionError);
        }
        let bytes = Bytes::from(output.bytes);
        cache.insert(key, bytes.clone());
        Ok(bytes)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DecompressionError;

impl AssetSource for &'static EmbeddedFrontend {
    fn get(&self, request: AssetRequest<'_>) -> Option<AssetResponse> {
        crate::request::respond(self, request)
    }
}

impl AssetProvider for &'static EmbeddedFrontend {
    fn version(&self) -> AssetVersion {
        AssetVersion::from(self.version)
    }

    fn render_tags(&self, _context: AssetContext<'_>) -> Result<AssetTags, AssetError> {
        Ok(AssetTags::new(self.tags.to_owned()))
    }

    fn source(&self) -> Option<Arc<dyn AssetSource>> {
        Some(Arc::new(*self))
    }

    fn public_path(&self) -> Option<&str> {
        Some(self.public_path)
    }
}
