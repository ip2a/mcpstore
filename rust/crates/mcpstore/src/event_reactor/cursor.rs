//! Cursor persistence: stores the last-processed ChangeFeed cursor per subscriber.

use openkeyv::{AsyncCompareAndSwap, AsyncKeyValue, Revision};

use crate::cache::codec;

const CURSOR_COLLECTION_SUFFIX: &str = "reactor:cursors";

/// Manages save/load of a subscriber's feed cursor in the openkeyv store.
pub(crate) struct CursorStore<S> {
    store: S,
    collection: String,
    subscriber_id: String,
}

impl<S> CursorStore<S>
where
    S: AsyncKeyValue + AsyncCompareAndSwap + Clone + Send + Sync,
{
    pub(crate) fn new(store: S, namespace: &str, subscriber_id: impl Into<String>) -> Self {
        Self {
            store,
            collection: format!("{namespace}:{CURSOR_COLLECTION_SUFFIX}"),
            subscriber_id: subscriber_id.into(),
        }
    }

    /// Load the last saved cursor, or None if this subscriber is new.
    pub(crate) async fn load(&self) -> Result<Option<String>, CursorError> {
        let value = self
            .store
            .get(&self.subscriber_id, Some(&self.collection))
            .await
            .map_err(CursorError::Store)?;
        match value {
            None => Ok(None),
            Some(v) => {
                let json = codec::value_to_json(v).map_err(CursorError::Codec)?;
                Ok(json
                    .get("cursor")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()))
            }
        }
    }

    /// Save a cursor, overwriting any previous value atomically via CAS.
    pub(crate) async fn save(&self, cursor: &str) -> Result<(), CursorError> {
        let payload = serde_json::json!({ "cursor": cursor });
        let value = codec::json_to_value(payload).map_err(CursorError::Codec)?;

        // CAS loop: read revision → compare_and_swap
        loop {
            let revisioned = self
                .store
                .get_with_revision(&self.subscriber_id, Some(&self.collection))
                .await
                .map_err(CursorError::Store)?;

            let expected: Option<&Revision> = revisioned.as_ref().map(|r| &r.revision);
            match self
                .store
                .compare_and_swap(
                    &self.subscriber_id,
                    expected,
                    value.clone(),
                    Some(&self.collection),
                    None,
                )
                .await
                .map_err(CursorError::Store)?
            {
                openkeyv::CompareAndSwapResult::Applied { .. } => return Ok(()),
                openkeyv::CompareAndSwapResult::Conflict { .. } => continue,
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum CursorError {
    Store(openkeyv::Error),
    Codec(crate::cache::CacheError),
}

impl std::fmt::Display for CursorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "store: {e}"),
            Self::Codec(e) => write!(f, "codec: {e}"),
        }
    }
}
