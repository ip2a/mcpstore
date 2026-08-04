#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum SourceMode {
    #[default]
    Local,
    Db,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheStorage {
    backend: String,
    url: Option<String>,
}

impl CacheStorage {
    pub fn memory() -> Self {
        Self::new("memory", None)
    }

    pub fn redis() -> Self {
        Self::new("redis", None)
    }

    pub fn openkeyv(backend: impl Into<String>, url: impl Into<String>) -> Self {
        Self::new(backend, Some(url.into()))
    }

    pub fn new(backend: impl Into<String>, url: Option<String>) -> Self {
        Self {
            backend: backend.into().to_ascii_lowercase(),
            url,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.backend
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn is_memory(&self) -> bool {
        self.backend == "memory"
    }

    pub fn with_fallback_url(mut self, url: Option<String>) -> Self {
        if self.url.is_none() {
            self.url = url;
        }
        self
    }
}

impl Default for CacheStorage {
    fn default() -> Self {
        Self::memory()
    }
}

pub type BackendKind = CacheStorage;

#[derive(Clone, Debug)]
pub struct StoreOptions {
    pub config_path: Option<String>,
    pub source_mode: SourceMode,
    pub backend: Option<CacheStorage>,
    pub redis_url: Option<String>,
    pub namespace: Option<String>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            source_mode: SourceMode::Local,
            backend: None,
            redis_url: None,
            namespace: None,
        }
    }
}
