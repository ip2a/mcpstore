#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum SourceMode {
    #[default]
    Local,
    Db,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum CacheStorage {
    #[default]
    Memory,
    Redis,
    OpenKeyvMemory,
    OpenKeyvRedis,
}

impl CacheStorage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Redis => "redis",
            Self::OpenKeyvMemory => "openkeyv_memory",
            Self::OpenKeyvRedis => "openkeyv_redis",
        }
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
