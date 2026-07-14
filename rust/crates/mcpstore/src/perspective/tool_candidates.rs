use crate::perspective::AvailableTool;
use crate::{InstanceId, Result, ScopeRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ToolCandidate {
    pub(super) instance_id: InstanceId,
    pub(super) service_name: String,
    pub(super) scope: ScopeRef,
    pub(super) tool_name: String,
    pub(super) display_name: String,
}

impl ToolCandidate {
    pub(super) fn from_tool(tool: &AvailableTool) -> Result<Self> {
        tool.validate()?;
        let service_name = tool.service_name()?.to_string();
        let tool_name = tool.tool_name()?.to_string();
        let display_name = tool.display_name().to_string();
        Ok(Self {
            instance_id: tool.instance_id,
            service_name,
            scope: tool.scope.clone(),
            tool_name,
            display_name,
        })
    }
}
