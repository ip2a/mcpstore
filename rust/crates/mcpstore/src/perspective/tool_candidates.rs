use crate::perspective::{generate_tool_global_name, AvailableTool};
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ToolCandidate {
    pub(super) service_name: String,
    pub(super) original_name: String,
    pub(super) display_name: String,
    pub(super) global_service_name: String,
    pub(super) global_tool_name: String,
}

impl ToolCandidate {
    pub(super) fn from_tool(tool: &AvailableTool) -> Result<Self> {
        let service_name = tool.service_name()?.to_string();
        let original_name = tool.canonical_name().to_string();
        let display_name = tool.display_name().to_string();
        let global_service_name = tool
            .global_service_name
            .clone()
            .unwrap_or_else(|| service_name.clone());
        let global_tool_name = match tool.global_tool_name.clone() {
            Some(name) => name,
            None => generate_tool_global_name(&global_service_name, &original_name)?,
        };
        Ok(Self {
            service_name,
            original_name,
            display_name,
            global_service_name,
            global_tool_name,
        })
    }
}
