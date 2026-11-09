//! Storage lifecycle manager for the daemon process.
//!
//! Consolidates telemetry SQLite initialization so daemon/gateway/API share
//! a single TelemetryLog instance instead of each opening the file independently.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::telemetry::TelemetryLog;

pub struct StorageManager {
    telemetry_path: PathBuf,
    telemetry: Option<Arc<TelemetryLog>>,
}

impl StorageManager {
    pub fn new(telemetry_path: impl AsRef<Path>) -> Self {
        Self {
            telemetry_path: telemetry_path.as_ref().to_path_buf(),
            telemetry: None,
        }
    }

    /// Open telemetry DB (creates schema if needed, starts writer thread).
    /// Fails loud: a daemon that can't record its calls points at broken storage.
    pub fn init_telemetry(&mut self) -> Result<(), String> {
        if self.telemetry.is_some() {
            return Ok(());
        }
        let log = TelemetryLog::open(&self.telemetry_path)
            .map_err(|e| format!("telemetry DB: {e}"))?;
        self.telemetry = Some(log);
        Ok(())
    }

    /// Get the shared telemetry sink (panics if not initialized).
    pub fn telemetry(&self) -> Arc<TelemetryLog> {
        self.telemetry
            .as_ref()
            .expect("StorageManager::init_telemetry not called")
            .clone()
    }

    pub fn telemetry_path(&self) -> &Path {
        &self.telemetry_path
    }
}
