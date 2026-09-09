use clap::{Args, Subcommand};
use serde_json::json;
use std::time::{Duration, Instant};

use crate::commands::mcp::open_store;
use crate::daemon::protocol::KernelOperation;
use crate::error::OutputFormat;
use crate::store_args::StoreSourceArgs;
use crate::BoxErr;

#[derive(Args)]
pub struct RequestArgs {
    #[command(subcommand)]
    pub action: RequestAction,
}

#[derive(Subcommand)]
pub enum RequestAction {
    List(RequestListArgs),
    Get(RequestTargetArgs),
    Wait(RequestWaitArgs),
}

#[derive(Args)]
pub struct RequestListArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,
}

#[derive(Args)]
pub struct RequestTargetArgs {
    pub request_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,
}

#[derive(Args)]
pub struct RequestWaitArgs {
    pub request_id: String,
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,
    #[arg(long, default_value_t = 30)]
    pub timeout: u64,
}

pub async fn run(args: RequestArgs, embedded: bool) -> Result<(), BoxErr> {
    match args.action {
        RequestAction::List(args) => list(args, embedded).await,
        RequestAction::Get(args) => get(args, embedded).await,
        RequestAction::Wait(args) => wait(args, embedded).await,
    }
}

async fn get_request(
    args: &RequestTargetArgs,
    embedded: bool,
) -> mcpstore::Result<serde_json::Value> {
    let mut access = open_store(&args.store, embedded).await?;
    access
        .request(
            KernelOperation::ControlRequestGet,
            json!({"request_id": args.request_id}),
        )
        .await
}

async fn list(args: RequestListArgs, embedded: bool) -> Result<(), BoxErr> {
    let mut access = open_store(&args.store, embedded).await?;
    let result = access
        .request(KernelOperation::ControlRequestList, json!({}))
        .await?;
    match args.output {
        OutputFormat::Human => {
            println!("[Requests] total={}", result["total"]);
            for request in result["requests"].as_array().into_iter().flatten() {
                println!(
                    "  - {} {} {}",
                    request["id"].as_str().unwrap_or("?"),
                    request["type"].as_str().unwrap_or("?"),
                    request["status"].as_str().unwrap_or("?")
                );
            }
        }
        format => crate::commands::mcp::emit_call_value(format, result)?,
    }
    Ok(())
}

async fn get(args: RequestTargetArgs, embedded: bool) -> Result<(), BoxErr> {
    let request = get_request(&args, embedded).await?;
    match args.output {
        OutputFormat::Human => {
            println!(
                "[Request] {} {} {}",
                request["id"].as_str().unwrap_or("?"),
                request["type"].as_str().unwrap_or("?"),
                request["status"].as_str().unwrap_or("?")
            );
        }
        format => crate::commands::mcp::emit_call_value(format, request)?,
    }
    Ok(())
}

async fn wait(args: RequestWaitArgs, embedded: bool) -> Result<(), BoxErr> {
    let deadline = Instant::now() + Duration::from_secs(args.timeout);
    loop {
        let request = get_request(
            &RequestTargetArgs {
                request_id: args.request_id.clone(),
                store: args.store.clone(),
                output: args.output,
            },
            embedded,
        )
        .await?;
        if matches!(
            request["status"].as_str(),
            Some("applied") | Some("rejected")
        ) {
            return get(
                RequestTargetArgs {
                    request_id: args.request_id,
                    store: args.store,
                    output: args.output,
                },
                embedded,
            )
            .await;
        }
        if Instant::now() >= deadline {
            let previous_status = request["status"].as_str().unwrap_or("?").to_string();
            let mut payload = request;
            payload["status"] = json!("timeout");
            if args.output == OutputFormat::Human {
                println!(
                    "[Timeout] request_id={} status={}",
                    args.request_id, previous_status
                );
            } else {
                crate::commands::mcp::emit_call_value(args.output, payload)?;
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
