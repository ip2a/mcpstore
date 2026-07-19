use clap::{Args, Subcommand, ValueEnum};
use mcpstore::transport::TransportError;
use mcpstore::{InstanceId, McpCompletionReference, McpCompletionRequest, StoreError};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::{
    commands::mcp::parse_instance_id,
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum ProtocolOutputFormat {
    #[default]
    Human,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ProtocolErrorCode {
    InvalidInput,
    ServiceNotFound,
    ConnectionFailed,
    AuthenticationRequired,
    CapabilityUnsupported,
    ProtocolFailed,
    CommandFailed,
}

impl ProtocolErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::ServiceNotFound => "service_not_found",
            Self::ConnectionFailed => "connection_failed",
            Self::AuthenticationRequired => "authentication_required",
            Self::CapabilityUnsupported => "capability_unsupported",
            Self::ProtocolFailed => "protocol_failed",
            Self::CommandFailed => "protocol_command_failed",
        }
    }

    fn exit_code(self) -> i32 {
        match self {
            Self::InvalidInput => 2,
            Self::ServiceNotFound => 10,
            Self::ConnectionFailed => 11,
            Self::AuthenticationRequired => 12,
            Self::CapabilityUnsupported => 20,
            Self::ProtocolFailed => 34,
            Self::CommandFailed => 1,
        }
    }
}

#[derive(Debug)]
pub struct ProtocolCommandError {
    format: ProtocolOutputFormat,
    code: ProtocolErrorCode,
    message: String,
    instance_id: Option<InstanceId>,
}

impl ProtocolCommandError {
    fn new(
        format: ProtocolOutputFormat,
        code: ProtocolErrorCode,
        message: impl Into<String>,
        instance_id: Option<InstanceId>,
    ) -> Self {
        Self {
            format,
            code,
            message: message.into(),
            instance_id,
        }
    }

    fn from_store(
        error: StoreError,
        format: ProtocolOutputFormat,
        instance_id: InstanceId,
    ) -> Self {
        let code = match &error {
            StoreError::ToolNotAvailable { .. } => ProtocolErrorCode::InvalidInput,
            StoreError::ServiceNotFound(_) => ProtocolErrorCode::ServiceNotFound,
            StoreError::Auth(_) => ProtocolErrorCode::AuthenticationRequired,
            StoreError::Transport(error) => match error {
                TransportError::InvalidInput(_) => ProtocolErrorCode::InvalidInput,
                TransportError::AuthRequired(_) | TransportError::InsufficientScope { .. } => {
                    ProtocolErrorCode::AuthenticationRequired
                }
                TransportError::CapabilityUnsupported { .. } => {
                    ProtocolErrorCode::CapabilityUnsupported
                }
                TransportError::ConnectionFailed(_)
                | TransportError::NotConnected(_)
                | TransportError::Io(_) => ProtocolErrorCode::ConnectionFailed,
                TransportError::Protocol(_) => ProtocolErrorCode::ProtocolFailed,
                _ => ProtocolErrorCode::CommandFailed,
            },
            StoreError::Cache(_)
            | StoreError::Config(_)
            | StoreError::State(_)
            | StoreError::Other(_) => ProtocolErrorCode::CommandFailed,
        };
        Self::new(format, code, error.to_string(), Some(instance_id))
    }

    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    fn json_value(&self) -> Value {
        json!({
            "event": "protocol.failed",
            "error": {
                "code": self.code.as_str(),
                "message": self.message,
            },
            "instance_id": self.instance_id,
        })
    }
}

impl std::fmt::Display for ProtocolCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.format {
            ProtocolOutputFormat::Human => {
                write!(formatter, "{}: {}", self.code.as_str(), self.message)
            }
            ProtocolOutputFormat::Json | ProtocolOutputFormat::Jsonl => {
                self.json_value().fmt(formatter)
            }
        }
    }
}

impl std::error::Error for ProtocolCommandError {}

#[derive(Debug, Clone, Args)]
pub struct ProtocolOutputArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = ProtocolOutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: ProtocolOutputFormat,
}

#[derive(Debug, Clone, Args)]
pub struct ProtocolInstanceArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[command(flatten)]
    pub output: ProtocolOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ResourceAction {
    List(ProtocolInstanceArgs),
    Templates(ProtocolInstanceArgs),
    Read(ResourceReadArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ResourceArgs {
    #[command(subcommand)]
    pub action: ResourceAction,
}

#[derive(Debug, Clone, Args)]
pub struct ResourceReadArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[arg(help = "Resource URI")]
    pub uri: String,
    #[command(flatten)]
    pub output: ProtocolOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PromptAction {
    List(ProtocolInstanceArgs),
    Get(PromptGetArgs),
}

#[derive(Debug, Clone, Args)]
pub struct PromptArgs {
    #[command(subcommand)]
    pub action: PromptAction,
}

#[derive(Debug, Clone, Args)]
pub struct PromptGetArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[arg(help = "Prompt name")]
    pub prompt_name: String,
    #[arg(long, default_value = "{}", help = "Prompt arguments JSON object")]
    pub arguments: String,
    #[command(flatten)]
    pub output: ProtocolOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum CompletionReferenceKind {
    Prompt,
    Resource,
}

#[derive(Debug, Clone, Args)]
pub struct CompleteArgs {
    #[arg(help = "Service instance ID")]
    pub instance_id: String,
    #[arg(
        long = "reference-kind",
        value_enum,
        help = "Completion reference kind"
    )]
    pub reference_kind: CompletionReferenceKind,
    #[arg(long = "reference", help = "Prompt name or resource URI template")]
    pub reference: String,
    #[arg(long = "argument-name", help = "Argument name")]
    pub argument_name: String,
    #[arg(long, help = "Partial argument value")]
    pub value: String,
    #[arg(long, default_value = "{}", help = "Completion context JSON object")]
    pub context: String,
    #[command(flatten)]
    pub output: ProtocolOutputArgs,
    #[command(flatten)]
    pub store: StoreSourceArgs,
}

pub async fn run_resource(args: ResourceArgs) -> Result<(), BoxErr> {
    execute_resource(args)
        .await
        .map_err(|error| Box::new(error) as BoxErr)
}

pub async fn run_prompt(args: PromptArgs) -> Result<(), BoxErr> {
    execute_prompt(args)
        .await
        .map_err(|error| Box::new(error) as BoxErr)
}

pub async fn complete(args: CompleteArgs) -> Result<(), BoxErr> {
    execute_complete(args)
        .await
        .map_err(|error| Box::new(error) as BoxErr)
}

async fn execute_resource(args: ResourceArgs) -> Result<(), ProtocolCommandError> {
    match args.action {
        ResourceAction::List(args) => execute_resource_list(args).await,
        ResourceAction::Templates(args) => execute_resource_templates(args).await,
        ResourceAction::Read(args) => execute_resource_read(args).await,
    }
}

async fn execute_resource_list(args: ProtocolInstanceArgs) -> Result<(), ProtocolCommandError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            error.to_string(),
            None,
        )
    })?;
    let store = build_store(&args.store).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            Some(instance_id),
        )
    })?;
    store.load_from_source().await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let resources = store.list_resources(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let total = resources.len();
    let value = json!({
        "event": "resource.listed",
        "instance_id": instance_id,
        "resources": resources,
        "total": total,
    });
    emit(
        args.output.output,
        format_resource_list(instance_id, &resources, total),
        value,
    )
}

async fn execute_resource_templates(
    args: ProtocolInstanceArgs,
) -> Result<(), ProtocolCommandError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            error.to_string(),
            None,
        )
    })?;
    let store = build_store(&args.store).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            Some(instance_id),
        )
    })?;
    store.load_from_source().await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let templates = store
        .list_resource_templates(instance_id)
        .await
        .map_err(|error| {
            ProtocolCommandError::from_store(error, args.output.output, instance_id)
        })?;
    let total = templates.len();
    let value = json!({
        "event": "resource.templates_listed",
        "instance_id": instance_id,
        "resource_templates": templates,
        "total": total,
    });
    emit(
        args.output.output,
        format!("[Resource Templates] instance={instance_id} count={total}"),
        value,
    )
}

async fn execute_resource_read(args: ResourceReadArgs) -> Result<(), ProtocolCommandError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            error.to_string(),
            None,
        )
    })?;
    if args.uri.trim().is_empty() {
        return Err(ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            "resource URI must not be empty",
            Some(instance_id),
        ));
    }
    let store = build_store(&args.store).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            Some(instance_id),
        )
    })?;
    store.load_from_source().await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let resource = store
        .read_resource(instance_id, &args.uri)
        .await
        .map_err(|error| {
            ProtocolCommandError::from_store(error, args.output.output, instance_id)
        })?;
    let value = json!({
        "event": "resource.read",
        "instance_id": instance_id,
        "uri": args.uri,
        "resource": resource,
    });
    emit(args.output.output, value["resource"].to_string(), value)
}

async fn execute_prompt(args: PromptArgs) -> Result<(), ProtocolCommandError> {
    match args.action {
        PromptAction::List(args) => execute_prompt_list(args).await,
        PromptAction::Get(args) => execute_prompt_get(args).await,
    }
}

async fn execute_prompt_list(args: ProtocolInstanceArgs) -> Result<(), ProtocolCommandError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            error.to_string(),
            None,
        )
    })?;
    let store = build_store(&args.store).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            Some(instance_id),
        )
    })?;
    store.load_from_source().await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let prompts = store.list_prompts(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let total = prompts.len();
    let value = json!({
        "event": "prompt.listed",
        "instance_id": instance_id,
        "prompts": prompts,
        "total": total,
    });
    emit(
        args.output.output,
        format_prompt_list(instance_id, &prompts, total),
        value,
    )
}

async fn execute_prompt_get(args: PromptGetArgs) -> Result<(), ProtocolCommandError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            error.to_string(),
            None,
        )
    })?;
    let arguments = parse_object(
        &args.arguments,
        args.output.output,
        Some(instance_id),
        "prompt arguments",
    )?;
    if args.prompt_name.trim().is_empty() {
        return Err(ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            "prompt name must not be empty",
            Some(instance_id),
        ));
    }
    let store = build_store(&args.store).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            Some(instance_id),
        )
    })?;
    store.load_from_source().await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let prompt = store
        .get_prompt(instance_id, &args.prompt_name, arguments)
        .await
        .map_err(|error| {
            ProtocolCommandError::from_store(error, args.output.output, instance_id)
        })?;
    let value = json!({
        "event": "prompt.get",
        "instance_id": instance_id,
        "prompt_name": args.prompt_name,
        "prompt": prompt,
    });
    emit(args.output.output, value["prompt"].to_string(), value)
}

async fn execute_complete(args: CompleteArgs) -> Result<(), ProtocolCommandError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            error.to_string(),
            None,
        )
    })?;
    if args.reference.trim().is_empty() || args.argument_name.trim().is_empty() {
        return Err(ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::InvalidInput,
            "reference and argument-name must not be empty",
            Some(instance_id),
        ));
    }
    let context =
        serde_json::from_str::<HashMap<String, String>>(&args.context).map_err(|error| {
            ProtocolCommandError::new(
                args.output.output,
                ProtocolErrorCode::InvalidInput,
                format!("completion context must be a JSON object with string values: {error}"),
                Some(instance_id),
            )
        })?;
    let reference = match args.reference_kind {
        CompletionReferenceKind::Prompt => McpCompletionReference::Prompt {
            name: args.reference.clone(),
        },
        CompletionReferenceKind::Resource => McpCompletionReference::Resource {
            uri_template: args.reference.clone(),
        },
    };
    let request = McpCompletionRequest {
        reference,
        argument_name: args.argument_name,
        value: args.value,
        context,
    };
    let store = build_store(&args.store).map_err(|error| {
        ProtocolCommandError::new(
            args.output.output,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            Some(instance_id),
        )
    })?;
    store.load_from_source().await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        ProtocolCommandError::from_store(error, args.output.output, instance_id)
    })?;
    let completion = store
        .complete_mcp_argument(instance_id, request)
        .await
        .map_err(|error| {
            ProtocolCommandError::from_store(error, args.output.output, instance_id)
        })?;
    let value = json!({
        "event": "completion.completed",
        "instance_id": instance_id,
        "completion": completion,
    });
    emit(args.output.output, value["completion"].to_string(), value)
}

fn parse_object(
    input: &str,
    format: ProtocolOutputFormat,
    instance_id: Option<InstanceId>,
    name: &str,
) -> Result<Value, ProtocolCommandError> {
    let value: Value = serde_json::from_str(input).map_err(|error| {
        ProtocolCommandError::new(
            format,
            ProtocolErrorCode::InvalidInput,
            format!("{name} must be a JSON object: {error}"),
            instance_id,
        )
    })?;
    if !value.is_object() {
        return Err(ProtocolCommandError::new(
            format,
            ProtocolErrorCode::InvalidInput,
            format!("{name} must be a JSON object"),
            instance_id,
        ));
    }
    Ok(value)
}

fn format_resource_list(
    instance_id: InstanceId,
    resources: &[mcpstore::DiscoveredResource],
    total: usize,
) -> String {
    let mut output = format!("[Resources] instance={instance_id} count={total}");
    for resource in resources {
        output.push_str(&format!(
            "\n- {}  uri={}{}",
            resource.name,
            resource.uri,
            resource
                .mime_type
                .as_deref()
                .map(|mime| format!("  mime={mime}"))
                .unwrap_or_default()
        ));
    }
    output
}

fn format_prompt_list(
    instance_id: InstanceId,
    prompts: &[mcpstore::DiscoveredPrompt],
    total: usize,
) -> String {
    let mut output = format!("[Prompts] instance={instance_id} count={total}");
    for prompt in prompts {
        let arguments = prompt
            .arguments
            .as_ref()
            .and_then(Value::as_array)
            .map(|arguments| {
                arguments
                    .iter()
                    .filter_map(|argument| argument.get("name").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|arguments| !arguments.is_empty())
            .map(|arguments| format!("  arguments={arguments}"))
            .unwrap_or_default();
        output.push_str(&format!("\n- {}{}", prompt.name, arguments));
    }
    output
}

fn emit(
    format: ProtocolOutputFormat,
    human: String,
    value: Value,
) -> Result<(), ProtocolCommandError> {
    let encoded = match format {
        ProtocolOutputFormat::Human => {
            println!("{human}");
            return Ok(());
        }
        ProtocolOutputFormat::Json => serde_json::to_string_pretty(&value),
        ProtocolOutputFormat::Jsonl => serde_json::to_string(&value),
    }
    .map_err(|error| {
        ProtocolCommandError::new(
            format,
            ProtocolErrorCode::CommandFailed,
            error.to_string(),
            None,
        )
    })?;
    println!("{encoded}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_parser_rejects_non_object() {
        let error = parse_object("[]", ProtocolOutputFormat::Jsonl, None, "arguments").unwrap_err();
        assert_eq!(error.code, ProtocolErrorCode::InvalidInput);
        assert_eq!(error.exit_code(), 2);
    }
}
