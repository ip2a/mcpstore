use super::*;

impl MCPStore {
    pub(super) async fn collect_store_tools_scoped(&self) -> Result<Vec<serde_json::Value>> {
        let mut services = self.list_services().await;
        services.sort_by(|left, right| left.name.cmp(&right.name));

        let mut tools = Vec::new();
        for service in services {
            let mut service_tools = service.tools.clone();
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = generate_tool_global_name(&service.name, &original_name)?;
                tools.push(Self::tool_payload_value(
                    displayed_name,
                    original_name,
                    service.name.clone(),
                    service.name.clone(),
                    tool.description,
                    tool.schema,
                )?);
            }
        }
        Ok(tools)
    }

    pub(super) async fn collect_store_tool_descriptions_scoped(
        &self,
    ) -> Result<Vec<ScopedToolEntry>> {
        let mut services = self.list_services().await;
        services.sort_by(|left, right| left.name.cmp(&right.name));

        let mut tools = Vec::new();
        for service in services {
            let mut service_tools = service.tools.clone();
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = generate_tool_global_name(&service.name, &original_name)?;
                tools.push(Self::scoped_tool_entry(
                    displayed_name,
                    original_name,
                    service.name.clone(),
                    service.name.clone(),
                    tool.description,
                    tool.schema,
                )?);
            }
        }
        Ok(tools)
    }

    pub(super) async fn collect_store_resources_scoped(
        &self,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(None).await?;
        targets.sort();

        let mut resources = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_resources = self.list_resources(&global_service_name).await?;
            service_resources.sort_by(|left, right| {
                Self::value_field(left, "uri").cmp(Self::value_field(right, "uri"))
            });
            for resource in service_resources {
                resources.push(Self::resource_payload_value(
                    resource,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(resources)
    }

    pub(super) async fn collect_agent_resources_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(Some(agent_id)).await?;
        targets.sort();

        let mut resources = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_resources = self.list_resources(&global_service_name).await?;
            service_resources.sort_by(|left, right| {
                Self::value_field(left, "uri").cmp(Self::value_field(right, "uri"))
            });
            for resource in service_resources {
                resources.push(Self::resource_payload_value(
                    resource,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(resources)
    }

    pub(super) async fn collect_store_resource_templates_scoped(
        &self,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(None).await?;
        targets.sort();

        let mut templates = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_templates = self.list_resource_templates(&global_service_name).await?;
            service_templates.sort_by(|left, right| {
                Self::value_field(left, "uriTemplate").cmp(Self::value_field(right, "uriTemplate"))
            });
            for template in service_templates {
                templates.push(Self::resource_template_payload_value(
                    template,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(templates)
    }

    pub(super) async fn collect_agent_resource_templates_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(Some(agent_id)).await?;
        targets.sort();

        let mut templates = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_templates = self.list_resource_templates(&global_service_name).await?;
            service_templates.sort_by(|left, right| {
                Self::value_field(left, "uriTemplate").cmp(Self::value_field(right, "uriTemplate"))
            });
            for template in service_templates {
                templates.push(Self::resource_template_payload_value(
                    template,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(templates)
    }

    pub(super) async fn collect_store_prompts_scoped(&self) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(None).await?;
        targets.sort();

        let mut prompts = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_prompts = self.list_prompts(&global_service_name).await?;
            service_prompts.sort_by(|left, right| {
                Self::value_field(left, "name").cmp(Self::value_field(right, "name"))
            });
            for prompt in service_prompts {
                let original_name = Self::required_value_field(&prompt, "name")?;
                let display_name = format!("{}_{}", display_service_name, original_name);
                prompts.push(Self::prompt_payload_value(
                    prompt,
                    Some(display_name),
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(prompts)
    }

    pub(super) async fn collect_agent_prompts_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(Some(agent_id)).await?;
        targets.sort();

        let mut prompts = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_prompts = self.list_prompts(&global_service_name).await?;
            service_prompts.sort_by(|left, right| {
                Self::value_field(left, "name").cmp(Self::value_field(right, "name"))
            });
            for prompt in service_prompts {
                let original_name = Self::required_value_field(&prompt, "name")?;
                let display_name = format!("{}_{}", display_service_name, original_name);
                prompts.push(Self::prompt_payload_value(
                    prompt,
                    Some(display_name),
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(prompts)
    }

    pub(super) async fn ensure_service_connected(&self, service_name: &str) -> Result<()> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        if !self.pool.is_connected(service_name).await {
            self.connect_service_internal(service_name, true).await?;
        }
        Ok(())
    }

    async fn scoped_service_bindings(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<(String, String)>> {
        self.refresh_from_db_if_needed().await?;
        match agent_id {
            None => {
                let mut services = self.list_services().await;
                services.sort_by(|left, right| left.name.cmp(&right.name));
                Ok(services
                    .into_iter()
                    .map(|service| (service.name.clone(), service.name))
                    .collect())
            }
            Some(agent_id) => {
                let mut service_names = self.list_agent_service_names(agent_id).await?;
                service_names.sort();

                let mut bindings = Vec::with_capacity(service_names.len());
                for global_service_name in service_names {
                    let service = self
                        .find_service(&global_service_name)
                        .await
                        .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                    bindings.push((service.original_name.clone(), global_service_name));
                }
                Ok(bindings)
            }
        }
    }

    pub(super) async fn resolve_scoped_service_binding(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<(String, String)> {
        self.refresh_from_db_if_needed().await?;
        match agent_id {
            None => {
                let service = self
                    .find_service(service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?;
                Ok((service.name.clone(), service.name))
            }
            Some(agent_id) => {
                let global_service_name = self
                    .resolve_service_name_for_agent(agent_id, service_name)
                    .await?;
                let service = self
                    .find_service(&global_service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                Ok((service.original_name.clone(), global_service_name))
            }
        }
    }

    pub(super) async fn resolve_resource_service_binding(
        &self,
        agent_id: Option<&str>,
        uri: &str,
        service_name: Option<&str>,
    ) -> Result<(String, String)> {
        if let Some(service_name) = service_name {
            return self
                .resolve_scoped_service_binding(agent_id, service_name)
                .await;
        }

        let mut matches = Vec::new();
        for (display_service_name, global_service_name) in
            self.scoped_service_bindings(agent_id).await?
        {
            let resources = self.list_resources(&global_service_name).await?;
            if resources
                .iter()
                .any(|resource| Self::value_field(resource, "uri") == uri)
            {
                matches.push((display_service_name, global_service_name));
            }
        }

        match matches.len() {
            0 => Err(StoreError::Other(format!("未找到资源: {uri}"))),
            1 => Ok(matches.remove(0)),
            _ => Err(StoreError::Other(format!(
                "资源 URI 存在歧义，请显式提供 service_name: {uri}"
            ))),
        }
    }

    pub(super) async fn resolve_prompt_binding(
        &self,
        agent_id: Option<&str>,
        prompt_name: &str,
        service_name: Option<&str>,
    ) -> Result<(String, String, String)> {
        if let Some(service_name) = service_name {
            let (display_service_name, global_service_name) = self
                .resolve_scoped_service_binding(agent_id, service_name)
                .await?;
            return Ok((
                display_service_name,
                global_service_name,
                prompt_name.to_string(),
            ));
        }

        let mut matches = Vec::new();
        for (display_service_name, global_service_name) in
            self.scoped_service_bindings(agent_id).await?
        {
            let prompts = self.list_prompts(&global_service_name).await?;
            for prompt in prompts {
                let original_name = Self::required_value_field(&prompt, "name")?;
                let display_name = format!("{}_{}", display_service_name, original_name);
                if prompt_name == original_name || prompt_name == display_name {
                    matches.push((
                        display_service_name.clone(),
                        global_service_name.clone(),
                        original_name,
                    ));
                }
            }
        }

        match matches.len() {
            0 => Err(StoreError::Other(format!("未找到 prompt: {prompt_name}"))),
            1 => Ok(matches.remove(0)),
            _ => Err(StoreError::Other(format!(
                "prompt 名称存在歧义，请显式提供 service_name: {prompt_name}"
            ))),
        }
    }

    pub(super) async fn collect_agent_tools_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut service_names = self.list_agent_service_names(agent_id).await?;
        service_names.sort();

        let mut tools = Vec::new();
        for global_service_name in service_names {
            let service = self
                .find_service(&global_service_name)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
            let local_service_name = service.original_name.clone();
            let mut service_tools = self.list_tools(&global_service_name).await?;
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = format!("{}_{}", local_service_name, original_name);
                tools.push(Self::tool_payload_value(
                    displayed_name,
                    original_name,
                    local_service_name.clone(),
                    global_service_name.clone(),
                    tool.description,
                    tool.input_schema,
                )?);
            }
        }
        Ok(tools)
    }

    pub(super) async fn collect_agent_tool_descriptions_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<ScopedToolEntry>> {
        let mut service_names = self.list_agent_service_names(agent_id).await?;
        service_names.sort();

        let mut tools = Vec::new();
        for global_service_name in service_names {
            let service = self
                .find_service(&global_service_name)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
            let mut service_tools = self.list_tools(&global_service_name).await?;
            service_tools.sort_by(|left, right| left.name.cmp(&right.name));
            for tool in service_tools {
                let original_name = tool.name.clone();
                let displayed_name = format!("{}_{}", service.original_name, original_name);
                tools.push(Self::scoped_tool_entry(
                    displayed_name,
                    original_name,
                    service.original_name.clone(),
                    global_service_name.clone(),
                    tool.description,
                    tool.input_schema,
                )?);
            }
        }
        Ok(tools)
    }
}
