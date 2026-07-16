//! Rule registration: when predicate + then reaction.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

/// The context passed to a Rule's [`When`] predicate.
#[derive(Debug, Clone)]
pub struct ChangeContext {
    /// Collection that changed, e.g. `"mcpstore:event:service.add.requested"`.
    pub collection: String,
    /// Key inside the collection.
    pub key: String,
    /// The current value stored at that key (None for deletes).
    pub value: Option<Value>,
}

/// The context passed to a Rule's [`Then`] reaction.
#[derive(Debug, Clone)]
pub struct ReactionContext {
    /// The collection that triggered this reaction.
    pub collection: String,
    /// The key that triggered this reaction.
    pub key: String,
    /// The current value (None for deletes).
    pub value: Option<Value>,
    /// Unique identifier for this change, derived from the store change cursor.
    pub change_id: String,
}

/// Result of executing a reaction.
#[derive(Debug)]
pub enum ReactionOutcome {
    /// Reaction completed successfully.
    Ok,
    /// Reaction failed but can be retried later.
    Retryable(String),
    /// Reaction failed permanently.
    Failed(String),
}

/// Boxed future returned by async predicates and reactions.
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Predicate: returns true if the rule matches this change.
pub type When = Arc<dyn Fn(ChangeContext) -> BoxFuture<bool> + Send + Sync>;

/// Reaction: executed when the predicate matches.
pub type Then = Arc<dyn Fn(ReactionContext) -> BoxFuture<ReactionOutcome> + Send + Sync>;

/// A registered rule: when this change matches, execute that reaction.
///
/// The `id` must be stable across restarts because it forms part of the
/// idempotency key `(change_id, rule_id)` used for distributed claiming.
#[derive(Clone)]
pub struct Rule {
    id: String,
    when: When,
    then: Then,
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rule").field("id", &self.id).finish()
    }
}

impl Rule {
    /// Create a new rule with the given stable id.
    pub fn new(
        id: impl Into<String>,
        when: impl Fn(ChangeContext) -> BoxFuture<bool> + Send + Sync + 'static,
        then: impl Fn(ReactionContext) -> BoxFuture<ReactionOutcome> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            when: Arc::new(when),
            then: Arc::new(then),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn matches(&self, ctx: ChangeContext) -> bool {
        (self.when)(ctx).await
    }

    pub(crate) async fn execute(&self, ctx: ReactionContext) -> ReactionOutcome {
        (self.then)(ctx).await
    }
}
