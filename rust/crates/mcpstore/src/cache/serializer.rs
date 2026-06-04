//! Serialization utilities for cache values.
//!
//! All cache values are stored as JSON bytes. This module provides
//! thin wrappers around serde_json for consistency.

use serde::Serialize;

/// Convert a serializable value to a serde_json::Value.
pub fn to_value<T: Serialize>(value: &T) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(value)
}
