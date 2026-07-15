use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rmcp::model::{ElicitRequestParams, ElicitResult, ElicitationAction, ElicitationSchema};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::identity::InstanceId;

const ELICITATION_CHANNEL_CAPACITY: usize = 8;
const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpElicitationSessionOptions {
    pub response_timeout: Duration,
}

impl Default for McpElicitationSessionOptions {
    fn default() -> Self {
        Self {
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
        }
    }
}

impl McpElicitationSessionOptions {
    pub fn with_response_timeout(mut self, timeout: Duration) -> Self {
        self.response_timeout = timeout;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum McpElicitationRequestKind {
    Form {
        message: String,
        schema: ElicitationSchema,
    },
    Url {
        message: String,
        url: String,
        elicitation_id: String,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpElicitationResponseError {
    #[error("elicitation response channel is closed")]
    Closed,
    #[error("elicitation response timed out")]
    TimedOut,
    #[error("elicitation form response must be a JSON object")]
    FormResponseMustBeObject,
    #[error("elicitation form response is missing required field `{field}`")]
    MissingRequiredField { field: String },
    #[error("elicitation field `{field}` must be {expected}")]
    InvalidFieldType {
        field: String,
        expected: &'static str,
    },
    #[error("elicitation field `{field}` violates {rule}")]
    FieldConstraint { field: String, rule: &'static str },
    #[error("elicitation URL violates {rule}")]
    InvalidUrl { rule: &'static str },
    #[error("form elicitation requires response content")]
    MissingFormContent,
    #[error("URL elicitation must not include response content")]
    UnexpectedUrlContent,
}

pub struct McpElicitationRequest {
    instance_id: InstanceId,
    request_id: u64,
    deadline: Instant,
    kind: McpElicitationRequestKind,
    response: Option<oneshot::Sender<ElicitResult>>,
}

impl std::fmt::Debug for McpElicitationRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpElicitationRequest")
            .field("instance_id", &self.instance_id)
            .field("request_id", &self.request_id)
            .field("response_timeout", &self.response_timeout())
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl McpElicitationRequest {
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    pub fn response_timeout(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn kind(&self) -> &McpElicitationRequestKind {
        &self.kind
    }

    pub fn validate(&self, content: Option<&Value>) -> Result<(), McpElicitationResponseError> {
        match &self.kind {
            McpElicitationRequestKind::Form { schema, .. } => {
                let content = content.ok_or(McpElicitationResponseError::MissingFormContent)?;
                validate_form_response(schema, content)
            }
            McpElicitationRequestKind::Url { url, .. } => {
                if content.is_some() {
                    return Err(McpElicitationResponseError::UnexpectedUrlContent);
                }
                validate_handoff_url(url)
            }
        }
    }

    pub fn accept(mut self, content: Option<Value>) -> Result<(), McpElicitationResponseError> {
        self.validate(content.as_ref())?;
        self.send(ElicitationAction::Accept, content)
    }

    pub fn decline(mut self) -> Result<(), McpElicitationResponseError> {
        self.send(ElicitationAction::Decline, None)
    }

    pub fn cancel(mut self) -> Result<(), McpElicitationResponseError> {
        self.send(ElicitationAction::Cancel, None)
    }

    fn send(
        &mut self,
        action: ElicitationAction,
        content: Option<Value>,
    ) -> Result<(), McpElicitationResponseError> {
        if Instant::now() >= self.deadline {
            self.response.take();
            return Err(McpElicitationResponseError::TimedOut);
        }
        let result = match content {
            Some(content) => ElicitResult::new(action).with_content(content),
            None => ElicitResult::new(action),
        };
        self.response
            .take()
            .ok_or(McpElicitationResponseError::Closed)?
            .send(result)
            .map_err(|_| McpElicitationResponseError::Closed)
    }
}

impl Drop for McpElicitationRequest {
    fn drop(&mut self) {
        if let Some(response) = self.response.take() {
            let _ = response.send(cancel_result());
        }
    }
}

pub struct McpElicitationSession {
    instance_id: InstanceId,
    generation: u64,
    controller: McpElicitationController,
    requests: mpsc::Receiver<McpElicitationRequest>,
}

impl std::fmt::Debug for McpElicitationSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpElicitationSession")
            .field("instance_id", &self.instance_id)
            .finish_non_exhaustive()
    }
}

impl McpElicitationSession {
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub async fn next_request(&mut self) -> Option<McpElicitationRequest> {
        self.requests.recv().await
    }
}

impl Drop for McpElicitationSession {
    fn drop(&mut self) {
        self.controller.close_session(self.generation);
    }
}

#[derive(Clone)]
pub(crate) struct McpElicitationController {
    instance_id: InstanceId,
    state: Arc<Mutex<ControllerState>>,
}

struct ControllerState {
    next_generation: u64,
    next_request_id: u64,
    active: Option<ActiveSession>,
}

struct ActiveSession {
    generation: u64,
    response_timeout: Duration,
    requests: mpsc::Sender<McpElicitationRequest>,
}

impl McpElicitationController {
    pub(crate) fn new(instance_id: InstanceId) -> Self {
        Self {
            instance_id,
            state: Arc::new(Mutex::new(ControllerState {
                next_generation: 1,
                next_request_id: 1,
                active: None,
            })),
        }
    }

    pub(crate) fn open_session(
        &self,
        options: McpElicitationSessionOptions,
    ) -> Result<McpElicitationSession, ()> {
        let (requests, receiver) = mpsc::channel(ELICITATION_CHANNEL_CAPACITY);
        let mut state = self.state.lock().expect("elicitation controller poisoned");
        if state.active.is_some() {
            return Err(());
        }
        let generation = state.next_generation;
        state.next_generation += 1;
        state.active = Some(ActiveSession {
            generation,
            response_timeout: options.response_timeout,
            requests,
        });
        Ok(McpElicitationSession {
            instance_id: self.instance_id,
            generation,
            controller: self.clone(),
            requests: receiver,
        })
    }

    pub(crate) async fn elicit(&self, request: ElicitRequestParams) -> ElicitResult {
        let active = {
            let mut state = self.state.lock().expect("elicitation controller poisoned");
            let Some((requests, response_timeout)) = state
                .active
                .as_ref()
                .map(|active| (active.requests.clone(), active.response_timeout))
            else {
                return decline_result();
            };
            let request_id = state.next_request_id;
            state.next_request_id += 1;
            (requests, response_timeout, request_id)
        };
        let (response, receiver) = oneshot::channel();
        let Some(kind) = McpElicitationRequestKind::from_rmcp(request) else {
            return cancel_result();
        };
        let deadline = Instant::now() + active.1;
        let request = McpElicitationRequest {
            instance_id: self.instance_id,
            request_id: active.2,
            deadline,
            kind,
            response: Some(response),
        };
        if active.0.send(request).await.is_err() {
            return cancel_result();
        }
        match tokio::time::timeout(deadline.saturating_duration_since(Instant::now()), receiver)
            .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(_)) | Err(_) => cancel_result(),
        }
    }

    fn close_session(&self, generation: u64) {
        let mut state = self.state.lock().expect("elicitation controller poisoned");
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.generation == generation)
        {
            state.active = None;
        }
    }
}

impl McpElicitationRequestKind {
    fn from_rmcp(request: ElicitRequestParams) -> Option<Self> {
        match request {
            ElicitRequestParams::FormElicitationParams {
                message,
                requested_schema,
                ..
            } => Some(Self::Form {
                message,
                schema: requested_schema,
            }),
            ElicitRequestParams::UrlElicitationParams {
                message,
                url,
                elicitation_id,
                ..
            } => Some(Self::Url {
                message,
                url,
                elicitation_id,
            }),
            _ => None,
        }
    }
}

fn decline_result() -> ElicitResult {
    ElicitResult::new(ElicitationAction::Decline)
}

fn cancel_result() -> ElicitResult {
    ElicitResult::new(ElicitationAction::Cancel)
}

pub fn validate_form_response(
    schema: &ElicitationSchema,
    content: &Value,
) -> Result<(), McpElicitationResponseError> {
    let object = content
        .as_object()
        .ok_or(McpElicitationResponseError::FormResponseMustBeObject)?;
    if let Some(required) = &schema.required {
        for field in required {
            if !object.contains_key(field) {
                return Err(McpElicitationResponseError::MissingRequiredField {
                    field: field.clone(),
                });
            }
        }
    }
    let serialized = serde_json::to_value(schema)
        .expect("rmcp ElicitationSchema should always serialize successfully");
    let properties = serialized
        .get("properties")
        .and_then(Value::as_object)
        .expect("rmcp ElicitationSchema properties should be an object");
    for (field, value) in object {
        if let Some(property) = properties.get(field) {
            validate_property(field, property, value)?;
        }
    }
    Ok(())
}

fn validate_property(
    field: &str,
    schema: &Value,
    value: &Value,
) -> Result<(), McpElicitationResponseError> {
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => validate_string(field, schema, value),
        Some("number") => validate_number(field, schema, value),
        Some("integer") => validate_integer(field, schema, value),
        Some("boolean") => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(invalid_type(field, "a boolean"))
            }
        }
        Some("array") => validate_array(field, schema, value),
        _ => Err(McpElicitationResponseError::FieldConstraint {
            field: field.to_string(),
            rule: "a supported elicitation schema",
        }),
    }
}

fn validate_string(
    field: &str,
    schema: &Value,
    value: &Value,
) -> Result<(), McpElicitationResponseError> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid_type(field, "a string"))?;
    let length = text.chars().count() as u64;
    if schema
        .get("minLength")
        .and_then(Value::as_u64)
        .is_some_and(|minimum| length < minimum)
    {
        return Err(constraint(field, "minLength"));
    }
    if schema
        .get("maxLength")
        .and_then(Value::as_u64)
        .is_some_and(|maximum| length > maximum)
    {
        return Err(constraint(field, "maxLength"));
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !values
            .iter()
            .any(|candidate| candidate.as_str() == Some(text))
        {
            return Err(constraint(field, "enum"));
        }
    }
    if let Some(options) = schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(Value::as_array)
    {
        if !options.iter().any(|option| {
            option
                .get("const")
                .and_then(Value::as_str)
                .is_some_and(|candidate| candidate == text)
        }) {
            return Err(constraint(field, "enum"));
        }
    }
    match schema.get("format").and_then(Value::as_str) {
        Some("email") if !email_address::EmailAddress::is_valid(text) => {
            Err(constraint(field, "email format"))
        }
        Some("uri") if reqwest::Url::parse(text).is_err() => Err(constraint(field, "URI format")),
        Some("date") if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_err() => {
            Err(constraint(field, "date format"))
        }
        Some("date-time") if chrono::DateTime::parse_from_rfc3339(text).is_err() => {
            Err(constraint(field, "date-time format"))
        }
        _ => Ok(()),
    }
}

fn validate_number(
    field: &str,
    schema: &Value,
    value: &Value,
) -> Result<(), McpElicitationResponseError> {
    let number = value
        .as_f64()
        .ok_or_else(|| invalid_type(field, "a number"))?;
    if schema
        .get("minimum")
        .and_then(Value::as_f64)
        .is_some_and(|minimum| number < minimum)
    {
        return Err(constraint(field, "minimum"));
    }
    if schema
        .get("maximum")
        .and_then(Value::as_f64)
        .is_some_and(|maximum| number > maximum)
    {
        return Err(constraint(field, "maximum"));
    }
    Ok(())
}

fn validate_integer(
    field: &str,
    schema: &Value,
    value: &Value,
) -> Result<(), McpElicitationResponseError> {
    let integer = value
        .as_i64()
        .ok_or_else(|| invalid_type(field, "an integer"))?;
    if schema
        .get("minimum")
        .and_then(Value::as_i64)
        .is_some_and(|minimum| integer < minimum)
    {
        return Err(constraint(field, "minimum"));
    }
    if schema
        .get("maximum")
        .and_then(Value::as_i64)
        .is_some_and(|maximum| integer > maximum)
    {
        return Err(constraint(field, "maximum"));
    }
    Ok(())
}

fn validate_array(
    field: &str,
    schema: &Value,
    value: &Value,
) -> Result<(), McpElicitationResponseError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid_type(field, "an array"))?;
    if schema
        .get("minItems")
        .and_then(Value::as_u64)
        .is_some_and(|minimum| values.len() < minimum as usize)
    {
        return Err(constraint(field, "minItems"));
    }
    if schema
        .get("maxItems")
        .and_then(Value::as_u64)
        .is_some_and(|maximum| values.len() > maximum as usize)
    {
        return Err(constraint(field, "maxItems"));
    }
    let allowed = schema
        .get("items")
        .and_then(|items| {
            items
                .get("enum")
                .or_else(|| items.get("oneOf"))
                .or_else(|| items.get("anyOf"))
        })
        .and_then(Value::as_array);
    for value in values {
        let text = value
            .as_str()
            .ok_or_else(|| invalid_type(field, "an array of strings"))?;
        if let Some(allowed) = allowed {
            let matches = allowed.iter().any(|candidate| {
                candidate.as_str() == Some(text)
                    || candidate
                        .get("const")
                        .and_then(Value::as_str)
                        .is_some_and(|candidate| candidate == text)
            });
            if !matches {
                return Err(constraint(field, "enum"));
            }
        }
    }
    Ok(())
}

pub fn validate_handoff_url(url: &str) -> Result<(), McpElicitationResponseError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| McpElicitationResponseError::InvalidUrl { rule: "URL syntax" })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(McpElicitationResponseError::InvalidUrl {
            rule: "http or https scheme",
        });
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(McpElicitationResponseError::InvalidUrl {
            rule: "no embedded credentials",
        });
    }
    Ok(())
}

fn invalid_type(field: &str, expected: &'static str) -> McpElicitationResponseError {
    McpElicitationResponseError::InvalidFieldType {
        field: field.to_string(),
        expected,
    }
}

fn constraint(field: &str, rule: &'static str) -> McpElicitationResponseError {
    McpElicitationResponseError::FieldConstraint {
        field: field.to_string(),
        rule,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{
        ElicitationSchema, EnumSchema, IntegerSchema, PrimitiveSchemaDefinition, StringFormat,
        StringSchema,
    };
    use serde_json::json;

    fn instance_id(name: &str) -> InstanceId {
        crate::identity::ServiceInstanceKey::new(name, crate::identity::ScopeRef::Store)
            .instance_id()
    }

    fn form_request() -> ElicitRequestParams {
        ElicitRequestParams::FormElicitationParams {
            meta: None,
            message: "Provide account details".to_string(),
            requested_schema: ElicitationSchema::builder()
                .required_property(
                    "email",
                    PrimitiveSchemaDefinition::String(
                        StringSchema::new()
                            .min_length(3)
                            .format(StringFormat::Email),
                    ),
                )
                .required_property(
                    "age",
                    PrimitiveSchemaDefinition::Integer(IntegerSchema::new().range(18, 120)),
                )
                .build()
                .unwrap(),
        }
    }

    #[tokio::test]
    async fn no_session_declines_and_active_session_accepts() {
        let controller = McpElicitationController::new(instance_id("primary"));
        assert_eq!(
            controller.elicit(form_request()).await.action,
            ElicitationAction::Decline
        );

        let mut session = controller
            .open_session(McpElicitationSessionOptions::default())
            .unwrap();
        let response = tokio::spawn({
            let controller = controller.clone();
            async move { controller.elicit(form_request()).await }
        });
        let request = session.next_request().await.unwrap();
        request
            .accept(Some(json!({"email": "user@example.com", "age": 30})))
            .unwrap();
        let result = response.await.unwrap();
        assert_eq!(result.action, ElicitationAction::Accept);
        assert_eq!(result.content.unwrap()["age"], 30);
    }

    #[tokio::test]
    async fn decline_cancel_timeout_and_disconnect_are_explicit() {
        let controller = McpElicitationController::new(instance_id("lifecycle"));
        let mut session = controller
            .open_session(
                McpElicitationSessionOptions::default()
                    .with_response_timeout(Duration::from_millis(25)),
            )
            .unwrap();

        let declined = tokio::spawn({
            let controller = controller.clone();
            async move { controller.elicit(form_request()).await }
        });
        session.next_request().await.unwrap().decline().unwrap();
        assert_eq!(declined.await.unwrap().action, ElicitationAction::Decline);

        let cancelled = tokio::spawn({
            let controller = controller.clone();
            async move { controller.elicit(form_request()).await }
        });
        session.next_request().await.unwrap().cancel().unwrap();
        assert_eq!(cancelled.await.unwrap().action, ElicitationAction::Cancel);

        let timed_out = tokio::spawn({
            let controller = controller.clone();
            async move { controller.elicit(form_request()).await }
        });
        let _pending = session.next_request().await.unwrap();
        assert_eq!(timed_out.await.unwrap().action, ElicitationAction::Cancel);
        drop(_pending);

        let disconnected = tokio::spawn({
            let controller = controller.clone();
            async move { controller.elicit(form_request()).await }
        });
        let pending = session.next_request().await.unwrap();
        drop(session);
        drop(pending);
        assert_eq!(
            disconnected.await.unwrap().action,
            ElicitationAction::Cancel
        );
    }

    #[test]
    fn duplicate_sessions_are_rejected_and_instances_are_isolated() {
        let first = McpElicitationController::new(instance_id("first"));
        let second = McpElicitationController::new(instance_id("second"));
        let first_session = first
            .open_session(McpElicitationSessionOptions::default())
            .unwrap();
        assert!(first
            .open_session(McpElicitationSessionOptions::default())
            .is_err());
        let second_session = second
            .open_session(McpElicitationSessionOptions::default())
            .unwrap();
        assert_ne!(first_session.instance_id(), second_session.instance_id());
    }

    #[tokio::test]
    async fn concurrent_instances_keep_elicitation_requests_isolated() {
        let first = McpElicitationController::new(instance_id("concurrent-first"));
        let second = McpElicitationController::new(instance_id("concurrent-second"));
        let mut first_session = first
            .open_session(McpElicitationSessionOptions::default())
            .unwrap();
        let mut second_session = second
            .open_session(McpElicitationSessionOptions::default())
            .unwrap();

        let first_response = tokio::spawn({
            let first = first.clone();
            async move { first.elicit(form_request()).await }
        });
        let second_response = tokio::spawn({
            let second = second.clone();
            async move { second.elicit(form_request()).await }
        });

        let first_request = first_session.next_request().await.unwrap();
        let second_request = second_session.next_request().await.unwrap();
        assert_eq!(first_request.instance_id(), first_session.instance_id());
        assert_eq!(second_request.instance_id(), second_session.instance_id());
        assert_ne!(first_request.instance_id(), second_request.instance_id());

        first_request
            .accept(Some(json!({"email": "first@example.com", "age": 30})))
            .unwrap();
        second_request.decline().unwrap();

        assert_eq!(
            first_response.await.unwrap().action,
            ElicitationAction::Accept
        );
        assert_eq!(
            second_response.await.unwrap().action,
            ElicitationAction::Decline
        );
    }

    #[test]
    fn schema_validation_is_typed_bounded_and_secret_safe() {
        let schema = match form_request() {
            ElicitRequestParams::FormElicitationParams {
                requested_schema, ..
            } => requested_schema,
            _ => unreachable!(),
        };
        assert!(
            validate_form_response(&schema, &json!({"email": "user@example.com", "age": 30}))
                .is_ok()
        );

        for (input, expected) in [
            (json!({"age": 30}), "missing required field `email`"),
            (
                json!({"email": "very-secret-value", "age": "thirty"}),
                "field `age` must be an integer",
            ),
            (
                json!({"email": "very-secret-value", "age": 12}),
                "field `age` violates minimum",
            ),
            (
                json!({"email": "very-secret-value", "age": 30}),
                "field `email` violates email format",
            ),
        ] {
            let error = validate_form_response(&schema, &input).unwrap_err();
            let message = error.to_string();
            assert!(message.contains(expected));
            assert!(!message.contains("very-secret-value"));
        }
    }

    #[test]
    fn enum_schema_shapes_validate_single_and_multi_select() {
        let legacy = ElicitationSchema::from_json_schema({
            let mut schema = serde_json::Map::new();
            schema.insert("type".to_string(), json!("object"));
            schema.insert(
                "properties".to_string(),
                json!({"choice": {"type": "string", "enum": ["red", "blue"]}}),
            );
            schema
        })
        .unwrap();
        assert!(validate_form_response(&legacy, &json!({"choice": "red"})).is_ok());
        assert!(validate_form_response(&legacy, &json!({"choice": "green"})).is_err());

        let titled_single = ElicitationSchema::builder()
            .required_property(
                "choice",
                PrimitiveSchemaDefinition::Enum(
                    EnumSchema::builder(vec!["red".to_string(), "blue".to_string()])
                        .enum_titles(vec!["Red".to_string(), "Blue".to_string()])
                        .unwrap()
                        .build(),
                ),
            )
            .build()
            .unwrap();
        assert!(validate_form_response(&titled_single, &json!({"choice": "blue"})).is_ok());
        assert!(validate_form_response(&titled_single, &json!({"choice": "green"})).is_err());

        let untitled_multi = ElicitationSchema::builder()
            .required_property(
                "choices",
                PrimitiveSchemaDefinition::Enum(
                    EnumSchema::builder(vec![
                        "red".to_string(),
                        "blue".to_string(),
                        "green".to_string(),
                    ])
                    .multiselect()
                    .min_items(1)
                    .unwrap()
                    .max_items(2)
                    .unwrap()
                    .build(),
                ),
            )
            .build()
            .unwrap();
        assert!(
            validate_form_response(&untitled_multi, &json!({"choices": ["red", "blue"]})).is_ok()
        );
        assert!(validate_form_response(&untitled_multi, &json!({"choices": []})).is_err());
        assert!(validate_form_response(
            &untitled_multi,
            &json!({"choices": ["red", "blue", "green"]})
        )
        .is_err());
        assert!(validate_form_response(&untitled_multi, &json!({"choices": ["yellow"]})).is_err());

        let titled_multi = ElicitationSchema::builder()
            .required_property(
                "choices",
                PrimitiveSchemaDefinition::Enum(
                    EnumSchema::builder(vec!["red".to_string(), "blue".to_string()])
                        .multiselect()
                        .enum_titles(vec!["Red".to_string(), "Blue".to_string()])
                        .unwrap()
                        .build(),
                ),
            )
            .build()
            .unwrap();
        assert!(
            validate_form_response(&titled_multi, &json!({"choices": ["red", "blue"]})).is_ok()
        );
        assert!(validate_form_response(&titled_multi, &json!({"choices": ["yellow"]})).is_err());
    }

    #[test]
    fn handoff_url_rejects_unsafe_forms_without_echoing_them() {
        assert!(validate_handoff_url("https://example.com/continue").is_ok());
        for url in ["file:///tmp/secret", "https://user:secret@example.com/"] {
            let error = validate_handoff_url(url).unwrap_err().to_string();
            assert!(!error.contains(url));
            assert!(!error.contains("secret"));
        }
    }
}
