use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn scoped_tool_entry(
        displayed_name: String,
        tool_name: String,
        instance_id: InstanceId,
        service_name: String,
        scope: ScopeRef,
        title: Option<String>,
        description: String,
        input_schema: serde_json::Value,
        output_schema: Option<serde_json::Value>,
        annotations: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> ScopedToolEntry {
        ScopedToolEntry {
            name: displayed_name,
            tool_name,
            title,
            description,
            input_schema,
            output_schema,
            annotations,
            meta,
            instance_id,
            service_name,
            scope,
        }
    }
}
