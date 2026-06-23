mod names;
#[cfg(test)]
mod tests;
mod tool_candidates;
mod tool_matching;
mod tools;

pub use names::{
    generate_service_global_name, generate_tool_global_name, normalize_service_name,
    parse_agent_scoped, AgentScopedName, ServiceResolution, AGENT_SEPARATOR, GLOBAL_AGENT_STORE,
};
pub use tools::{resolve_tool, AvailableTool, ToolResolution};
