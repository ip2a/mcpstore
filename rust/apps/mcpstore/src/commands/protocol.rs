use clap::{Args, Subcommand, ValueEnum};
use mcpstore::{InstanceId, McpCompletionReference, McpCompletionRequest};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::{CliError, Domain, ErrorCode, OutputFormat};
use crate::{
    commands::mcp::parse_instance_id,
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

#[derive(Debug, Clone, Args)]
pub struct ProtocolOutputArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Human,
        help = "Output format: human, json, or jsonl"
    )]
    pub output: OutputFormat,
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

async fn execute_resource(args: ResourceArgs) -> Result<(), CliError> {
    match args.action {
        ResourceAction::List(args) => execute_resource_list(args).await,
        ResourceAction::Templates(args) => execute_resource_templates(args).await,
        ResourceAction::Read(args) => execute_resource_read(args).await,
    }
}

async fn execute_resource_list(args: ProtocolInstanceArgs) -> Result<(), CliError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::InvalidInput, error.to_string())
    })?;
    let store = build_store(&args.store).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
            .with("instance_id", instance_id.to_string())
    })?;
    store.load_from_source().await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    let resources = store.list_resources(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
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
) -> Result<(), CliError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::InvalidInput, error.to_string())
    })?;
    let store = build_store(&args.store).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
            .with("instance_id", instance_id.to_string())
    })?;
    store.load_from_source().await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    let templates = store
        .list_resource_templates(instance_id)
        .await
        .map_err(|error| {
            CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
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

async fn execute_resource_read(args: ResourceReadArgs) -> Result<(), CliError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::InvalidInput, error.to_string())
    })?;
    if args.uri.trim().is_empty() {
        return Err(CliError::new(
            args.output.output,
            Domain::Protocol,
            ErrorCode::InvalidInput,
            "resource URI must not be empty",
        )
        .with("instance_id", instance_id.to_string()));
    }
    let store = build_store(&args.store).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
            .with("instance_id", instance_id.to_string())
    })?;
    store.load_from_source().await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    let resource = store
        .read_resource(instance_id, &args.uri)
        .await
        .map_err(|error| {
            CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
        })?;
    let value = json!({
        "event": "resource.read",
        "instance_id": instance_id,
        "uri": args.uri,
        "resource": resource,
    });
    emit(args.output.output, value["resource"].to_string(), value)
}

async fn execute_prompt(args: PromptArgs) -> Result<(), CliError> {
    match args.action {
        PromptAction::List(args) => execute_prompt_list(args).await,
        PromptAction::Get(args) => execute_prompt_get(args).await,
    }
}

async fn execute_prompt_list(args: ProtocolInstanceArgs) -> Result<(), CliError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::InvalidInput, error.to_string())
    })?;
    let store = build_store(&args.store).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
            .with("instance_id", instance_id.to_string())
    })?;
    store.load_from_source().await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    let prompts = store.list_prompts(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
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

async fn execute_prompt_get(args: PromptGetArgs) -> Result<(), CliError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::InvalidInput, error.to_string())
    })?;
    let arguments = parse_object(
        &args.arguments,
        args.output.output,
        Some(instance_id),
        "prompt arguments",
    )?;
    if args.prompt_name.trim().is_empty() {
        return Err(CliError::new(
            args.output.output,
            Domain::Protocol,
            ErrorCode::InvalidInput,
            "prompt name must not be empty",
        )
        .with("instance_id", instance_id.to_string()));
    }
    let store = build_store(&args.store).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
            .with("instance_id", instance_id.to_string())
    })?;
    store.load_from_source().await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    let prompt = store
        .get_prompt(instance_id, &args.prompt_name, arguments)
        .await
        .map_err(|error| {
            CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
        })?;
    let value = json!({
        "event": "prompt.get",
        "instance_id": instance_id,
        "prompt_name": args.prompt_name,
        "prompt": prompt,
    });
    emit(args.output.output, value["prompt"].to_string(), value)
}

async fn execute_complete(args: CompleteArgs) -> Result<(), CliError> {
    let instance_id = parse_instance_id(&args.instance_id).map_err(|error| {
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::InvalidInput, error.to_string())
    })?;
    if args.reference.trim().is_empty() || args.argument_name.trim().is_empty() {
        return Err(CliError::new(
            args.output.output,
            Domain::Protocol,
            ErrorCode::InvalidInput,
            "reference and argument-name must not be empty",
        )
        .with("instance_id", instance_id.to_string()));
    }
    let context =
        serde_json::from_str::<HashMap<String, String>>(&args.context).map_err(|error| {
            CliError::new(
                args.output.output,
                Domain::Protocol,
                ErrorCode::InvalidInput,
                format!("completion context must be a JSON object with string values: {error}"),
            )
            .with("instance_id", instance_id.to_string())
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
        CliError::new(args.output.output, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
            .with("instance_id", instance_id.to_string())
    })?;
    store.load_from_source().await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    store.connect_service(instance_id).await.map_err(|error| {
        CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
    })?;
    let completion = store
        .complete_mcp_argument(instance_id, request)
        .await
        .map_err(|error| {
            CliError::from_store(&error, args.output.output, Domain::Protocol).with("instance_id", instance_id.to_string())
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
    format: OutputFormat,
    instance_id: Option<InstanceId>,
    name: &str,
) -> Result<Value, CliError> {
    let make_error = |message: String| {
        let err = CliError::new(format, Domain::Protocol, ErrorCode::InvalidInput, message);
        match instance_id {
            Some(id) => err.with("instance_id", id.to_string()),
            None => err,
        }
    };
    let value: Value = serde_json::from_str(input)
        .map_err(|error| make_error(format!("{name} must be a JSON object: {error}")))?;
    if !value.is_object() {
        return Err(make_error(format!("{name} must be a JSON object")));
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
    format: OutputFormat,
    human: String,
    value: Value,
) -> Result<(), CliError> {
    let encoded = match format {
        OutputFormat::Human => {
            println!("{human}");
            return Ok(());
        }
        OutputFormat::Json => serde_json::to_string_pretty(&value),
        OutputFormat::Jsonl => serde_json::to_string(&value),
    }
    .map_err(|error| {
        CliError::new(format, Domain::Protocol, ErrorCode::CommandFailed, error.to_string())
    })?;
    println!("{encoded}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_parser_rejects_non_object() {
        let error = parse_object("[]", OutputFormat::Jsonl, None, "arguments").unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput);
        assert_eq!(error.exit_code(), 2);
    }
}
