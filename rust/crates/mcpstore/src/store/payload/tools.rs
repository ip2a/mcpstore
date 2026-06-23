use crate::store::prelude::*;

impl MCPStore {
    pub(crate) fn tool_payload_value(
        displayed_name: String,
        original_name: String,
        service_name: String,
        global_service_name: String,
        description: String,
        schema: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let global_tool_name = generate_tool_global_name(&global_service_name, &original_name)?;
        let client_id = global_service_name.clone();
        Ok(serde_json::json!({
            "name": displayed_name,
            "original_name": original_name,
            "description": description,
            "schema": schema,
            "input_schema": schema,
            "service_name": service_name,
            "global_service_name": global_service_name,
            "service_global_name": global_service_name,
            "global_tool_name": global_tool_name,
            "client_id": client_id,
        }))
    }

    pub(crate) fn scoped_tool_entry(
        displayed_name: String,
        original_name: String,
        service_name: String,
        global_service_name: String,
        description: String,
        schema: serde_json::Value,
    ) -> Result<ScopedToolEntry> {
        let global_tool_name = generate_tool_global_name(&global_service_name, &original_name)?;
        let client_id = global_service_name.clone();
        Ok(ScopedToolEntry {
            name: displayed_name,
            original_name,
            description,
            schema: schema.clone(),
            input_schema: schema,
            service_name,
            service_global_name: global_service_name.clone(),
            global_service_name,
            global_tool_name,
            client_id,
        })
    }
}
