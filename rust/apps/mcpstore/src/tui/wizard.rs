//! Lightweight interactive mode (zi style): inline prompt driven, not full screen.
//! Reuses the StoreAccess/KernelOperation data layer; does not reuse the ratatui app state machine.
use std::collections::HashMap;

use inquire::{Confirm, Select, Text};
use mcpstore::{InstanceId, ScopeRef};
use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

use crate::store_args::{StoreAccess, StoreSourceArgs};
use crate::BoxErr;

pub fn run(args: &StoreSourceArgs, embedded: bool) -> Result<(), BoxErr> {
    let rt = crate::bootstrap::build_runtime()?;
    let mut access = rt.block_on(crate::store_args::open_store_access(args, embedded, None))?;

    let result = main_loop(&rt, &mut access);
    rt.block_on(access.close());
    result
}

fn main_loop(rt: &tokio::runtime::Runtime, access: &mut StoreAccess) -> Result<(), BoxErr> {
    loop {
        let action = Select::new(
            "mcpstore",
            vec!["Services", "Tool Test", "Agent Auth", "Quit"],
        )
        .prompt();

        match action {
            Ok("Services") => service_menu(rt, access)?,
            Ok("Tool Test") => tool_menu(rt, access)?,
            Ok("Agent Auth") => agent_menu(rt, access)?,
            _ => break,
        }
    }
    Ok(())
}

fn fuzzy_scorer(input: &str, value: &String, _label: &str, _idx: usize) -> Option<i64> {
    if input.is_empty() {
        return Some(0);
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(input, CaseMatching::Smart, Normalization::Smart);
    let mut haystack = Vec::new();
    pattern
        .score(Utf32Str::new(value, &mut haystack), &mut matcher)
        .map(i64::from)
}

fn list_services(
    rt: &tokio::runtime::Runtime,
    access: &mut StoreAccess,
) -> Result<Vec<ServiceRow>, BoxErr> {
    let result = rt.block_on(access.request(
        crate::daemon::protocol::KernelOperation::ListInstances,
        serde_json::json!({}),
    ))?;
    let entries: Vec<serde_json::Value> = serde_json::from_value(result["instances"].clone())?;
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        let instance: mcpstore::ServiceInstance =
            serde_json::from_value(entry["instance"].clone())?;
        let state: mcpstore::state::ServiceState = serde_json::from_value(entry["state"].clone())?;
        rows.push(ServiceRow {
            instance_id: instance.instance_id,
            name: instance.service_name,
            scope: instance.scope,
            transport: instance.transport,
            readiness: format!("{:?}", state.readiness.status),
            phase: format!("{:?}", state.phase),
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

struct ServiceRow {
    instance_id: InstanceId,
    name: String,
    scope: ScopeRef,
    transport: String,
    readiness: String,
    phase: String,
}

impl std::fmt::Display for ServiceRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scope = match &self.scope {
            ScopeRef::Store => "store".to_string(),
            ScopeRef::Agent { agent_id } => format!("agent:{agent_id}"),
        };
        write!(
            f,
            "{:<24} {:<8} {:<10} {:<10} {}",
            self.name, self.transport, self.readiness, self.phase, scope
        )
    }
}

fn service_menu(rt: &tokio::runtime::Runtime, access: &mut StoreAccess) -> Result<(), BoxErr> {
    loop {
        let mut services = list_services(rt, access)?;
        if services.is_empty() {
            println!("[Info] No services yet, add one first");
            if !add_service_flow(rt, access)? {
                return Ok(());
            }
            continue;
        }

        let mut items: Vec<String> = services.iter().map(ToString::to_string).collect();
        items.push("+ Add service".to_string());
        items.push("< Back".to_string());

        let choice = Select::new("Select service:", items.clone())
            .with_scorer(&fuzzy_scorer)
            .prompt();

        let Ok(choice) = choice else { return Ok(()) };
        let idx = items
            .iter()
            .position(|item| item == &choice)
            .unwrap_or(items.len() - 1);

        if idx == items.len() - 1 {
            return Ok(());
        }
        if idx == items.len() - 2 {
            add_service_flow(rt, access)?;
            continue;
        }

        let row = services.remove(idx);
        service_action_menu(rt, access, row)?;
    }
}

fn service_action_menu(
    rt: &tokio::runtime::Runtime,
    access: &mut StoreAccess,
    row: ServiceRow,
) -> Result<(), BoxErr> {
    let action = Select::new(
        &format!("{} :", row.name),
        vec!["Connect", "Disconnect", "Restart", "Delete", "< Back"],
    )
    .prompt();

    match action {
        Ok("Connect") => {
            let result = rt.block_on(access.request(
                crate::daemon::protocol::KernelOperation::ConnectService,
                serde_json::json!({"instance_id": row.instance_id.to_string()}),
            ));
            report(result, &format!("Connect {}", row.name));
        }
        Ok("Disconnect") => {
            let result = rt.block_on(access.request(
                crate::daemon::protocol::KernelOperation::DisconnectService,
                serde_json::json!({"instance_id": row.instance_id.to_string()}),
            ));
            report(result, &format!("Disconnect {}", row.name));
        }
        Ok("Restart") => {
            let result = rt.block_on(access.request(
                crate::daemon::protocol::KernelOperation::RestartService,
                serde_json::json!({"instance_id": row.instance_id.to_string()}),
            ));
            report(result, &format!("Restart {}", row.name));
        }
        Ok("Delete") => {
            let confirmed = Confirm::new(&format!("Delete service {}?", row.name))
                .with_default(false)
                .prompt()
                .unwrap_or(false);
            if confirmed {
                let result = rt.block_on(access.request(
                    crate::daemon::protocol::KernelOperation::RemoveServiceScope,
                    serde_json::json!({"service_name": row.name, "scope": row.scope}),
                ));
                report(result, &format!("Delete {}", row.name));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Returns false when the user cancels (the caller will not re-prompt).
fn add_service_flow(
    rt: &tokio::runtime::Runtime,
    access: &mut StoreAccess,
) -> Result<bool, BoxErr> {
    let mode = Select::new("Service type:", vec!["http", "stdio", "< Cancel"]).prompt();
    let mode = match mode {
        Ok(m) if m != "< Cancel" => m,
        _ => return Ok(false),
    };

    let name = Text::new("Service name:").prompt()?;
    if name.trim().is_empty() {
        println!("[Error] Service name cannot be empty");
        return Ok(true);
    }

    let mut config = mcpstore::config::ServerConfig::default();
    if mode == "http" {
        config.url = Some(
            Text::new("URL:")
                .with_default("http://127.0.0.1:8000/mcp")
                .prompt()?,
        );
        config.transport = Some("streamable-http".to_string());
    } else {
        let command_line = Text::new("Command (with args):")
            .with_default("npx -y @modelcontextprotocol/server-filesystem .")
            .prompt()?;
        let mut parts = command_line.split_whitespace();
        config.command = parts.next().map(ToString::to_string);
        config.args = parts.map(ToString::to_string).collect();
        config.transport = Some("stdio".to_string());
    }

    let scope = Select::new("Scope:", vec!["store", "agent"]).prompt()?;
    let target_scope = if scope == "agent" {
        let agent_id = Text::new("Agent ID:").prompt()?;
        ScopeRef::Agent { agent_id }
    } else {
        ScopeRef::Store
    };

    let result = rt.block_on(access.request(
        crate::daemon::protocol::KernelOperation::AddService,
        serde_json::json!({"name": name, "config": config, "scope": target_scope}),
    ));

    match result {
        Ok(value) => {
            println!("[Success] Added service {name}");
            if let Ok(instance_id) =
                serde_json::from_value::<InstanceId>(value["instance_id"].clone())
            {
                if Confirm::new("Connect now?")
                    .with_default(true)
                    .prompt()
                    .unwrap_or(false)
                {
                    let connect_result = rt.block_on(access.request(
                        crate::daemon::protocol::KernelOperation::ConnectService,
                        serde_json::json!({"instance_id": instance_id.to_string()}),
                    ));
                    report(connect_result, &format!("Connect {name}"));
                }
            }
        }
        Err(error) => println!("[Error] Failed to add service: {error}"),
    }
    Ok(true)
}

fn tool_menu(rt: &tokio::runtime::Runtime, access: &mut StoreAccess) -> Result<(), BoxErr> {
    let result = rt
        .block_on(access.request(
            crate::daemon::protocol::KernelOperation::ListAllTools,
            serde_json::json!({}),
        ))
        .unwrap_or(serde_json::json!({"tools": []}));

    let services = list_services(rt, access)?;
    let name_by_instance: HashMap<InstanceId, String> = services
        .into_iter()
        .map(|row| (row.instance_id, row.name))
        .collect();

    let tools: Vec<(InstanceId, mcpstore::ToolInfo)> = result["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            Some((
                serde_json::from_value(entry["instance_id"].clone()).ok()?,
                serde_json::from_value(entry["tool"].clone()).ok()?,
            ))
        })
        .collect();

    if tools.is_empty() {
        println!("[Info] No tools available (services must be connected first)");
        return Ok(());
    }

    let items: Vec<String> = tools
        .iter()
        .map(|(instance_id, tool)| {
            let service_name = name_by_instance
                .get(instance_id)
                .cloned()
                .unwrap_or_else(|| instance_id.to_string());
            format!("{service_name}/{}", tool.name)
        })
        .collect();

    let choice = Select::new("Select tool:", items.clone())
        .with_scorer(&fuzzy_scorer)
        .prompt();
    let Ok(choice) = choice else { return Ok(()) };
    let idx = items.iter().position(|item| item == &choice).unwrap_or(0);
    let (instance_id, tool) = &tools[idx];

    let args_input = Text::new("Args (JSON, empty for {}):")
        .prompt()
        .unwrap_or_default();
    let args: serde_json::Value = if args_input.trim().is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_str(&args_input) {
            Ok(value) => value,
            Err(error) => {
                println!("[Error] Args are not valid JSON: {error}");
                return Ok(());
            }
        }
    };

    let result = rt.block_on(access.request(
        crate::daemon::protocol::KernelOperation::CallTool,
        serde_json::json!({
            "instance_id": instance_id.to_string(),
            "tool_name": tool.name,
            "args": args,
        }),
    ));

    match result {
        Ok(value) => println!(
            "[Success] Call completed:\n{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        Err(error) => println!("[Error] Call failed: {error}"),
    }
    Ok(())
}

fn agent_menu(rt: &tokio::runtime::Runtime, access: &mut StoreAccess) -> Result<(), BoxErr> {
    let services = list_services(rt, access)?;
    let mut agents: Vec<String> = services
        .iter()
        .filter_map(|row| match &row.scope {
            ScopeRef::Agent { agent_id } => Some(agent_id.clone()),
            ScopeRef::Store => None,
        })
        .collect();
    agents.sort();
    agents.dedup();

    let mut items = agents.clone();
    items.push("+ Grant service to agent".to_string());
    items.push("< Back".to_string());

    let choice = Select::new("Agent:", items.clone()).prompt();
    let Ok(choice) = choice else { return Ok(()) };

    if choice == "< Back" {
        return Ok(());
    }
    if choice == "+ Grant service to agent" {
        let agent_id = Text::new("Agent ID:").prompt()?;
        let service_name = Text::new("Service name:").prompt()?;
        let result = rt.block_on(access.request(
            crate::daemon::protocol::KernelOperation::DeclareServiceScope,
            serde_json::json!({
                "service_name": service_name,
                "scope": ScopeRef::Agent { agent_id: agent_id.clone() },
                "descriptor": mcpstore::config::ScopeDescriptor::default(),
            }),
        ));
        report(result, &format!("Grant {service_name} to {agent_id}"));
        return Ok(());
    }

    let agent_id = choice;
    let agent_services: Vec<String> = services
        .iter()
        .filter(|row| matches!(&row.scope, ScopeRef::Agent { agent_id: id } if id == &agent_id))
        .map(|row| row.name.clone())
        .collect();

    let mut svc_items = agent_services.clone();
    svc_items.push("< Back".to_string());
    let service_choice = Select::new(&format!("Services of {agent_id}:"), svc_items).prompt();
    let Ok(service_name) = service_choice else {
        return Ok(());
    };
    if service_name == "< Back" {
        return Ok(());
    }

    if Confirm::new(&format!("Revoke {agent_id}'s access to {service_name}?"))
        .with_default(false)
        .prompt()
        .unwrap_or(false)
    {
        let result = rt.block_on(access.request(
            crate::daemon::protocol::KernelOperation::RemoveServiceScope,
            serde_json::json!({
                "service_name": service_name,
                "scope": ScopeRef::Agent { agent_id: agent_id.clone() },
            }),
        ));
        report(result, &format!("Revoke {service_name}"));
    }
    Ok(())
}

fn report(result: mcpstore::Result<serde_json::Value>, action: &str) {
    match result {
        Ok(_) => println!("[Success] {action}"),
        Err(error) => println!("[Error] {action} failed: {error}"),
    }
}
