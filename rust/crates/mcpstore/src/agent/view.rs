use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_services_scoped(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        match agent_id {
            None => {
                let mut services = self.list_services().await;
                services.sort_by(|left, right| left.name.cmp(&right.name));
                Ok(services
                    .into_iter()
                    .map(|service| Self::service_payload_value(service, false))
                    .collect())
            }
            Some(agent_id) => {
                let mut service_names = self.list_agent_service_names(agent_id).await?;
                service_names.sort();

                let mut services = Vec::with_capacity(service_names.len());
                for global_service_name in service_names {
                    let service = self
                        .find_service(&global_service_name)
                        .await
                        .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                    services.push(Self::service_payload_value(service, true));
                }
                Ok(services)
            }
        }
    }

    pub async fn list_service_entries_scoped(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<ScopedServiceEntry>> {
        self.refresh_from_db_if_needed().await?;
        match agent_id {
            None => {
                let mut services = self.list_services().await;
                services.sort_by(|left, right| left.name.cmp(&right.name));
                Ok(services
                    .into_iter()
                    .map(|service| Self::scoped_service_entry(service, false))
                    .collect())
            }
            Some(agent_id) => {
                let mut service_names = self.list_agent_service_names(agent_id).await?;
                service_names.sort();

                let mut services = Vec::with_capacity(service_names.len());
                for global_service_name in service_names {
                    let service = self
                        .find_service(&global_service_name)
                        .await
                        .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                    services.push(Self::scoped_service_entry(service, true));
                }
                Ok(services)
            }
        }
    }

    pub async fn service_info_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<serde_json::Value> {
        self.refresh_from_db_if_needed().await?;
        let service = match agent_id {
            None => self
                .find_service(service_name)
                .await
                .ok_or_else(|| StoreError::ServiceNotFound(service_name.to_string()))?,
            Some(agent_id) => {
                let global_service_name = self
                    .resolve_service_name_for_agent(agent_id, service_name)
                    .await?;
                self.find_service(&global_service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.to_string()))?
            }
        };
        Ok(Self::service_payload_value(service, agent_id.is_some()))
    }

    pub async fn list_tools_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        match (agent_id, service_name) {
            (None, Some(service_name)) => {
                let tools = self.list_tools(service_name).await?;
                let mut payload = Vec::with_capacity(tools.len());
                for tool in tools {
                    payload.push(Self::tool_payload_value(
                        tool.name.clone(),
                        tool.name,
                        service_name.to_string(),
                        service_name.to_string(),
                        tool.description,
                        tool.input_schema,
                    )?);
                }
                Ok(payload)
            }
            (None, None) => self.collect_store_tools_scoped().await,
            (Some(agent_id), Some(service_name)) => {
                let global_service_name = self
                    .resolve_service_name_for_agent(agent_id, service_name)
                    .await?;
                let service = self
                    .find_service(&global_service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                let tools = self.list_tools(&global_service_name).await?;
                let mut payload = Vec::with_capacity(tools.len());
                for tool in tools {
                    payload.push(Self::tool_payload_value(
                        tool.name.clone(),
                        tool.name,
                        service.original_name.clone(),
                        global_service_name.clone(),
                        tool.description,
                        tool.input_schema,
                    )?);
                }
                Ok(payload)
            }
            (Some(agent_id), None) => self.collect_agent_tools_scoped(agent_id).await,
        }
    }

    pub async fn list_tool_entries_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<ScopedToolEntry>> {
        self.refresh_from_db_if_needed().await?;
        match (agent_id, service_name) {
            (None, Some(service_name)) => {
                let mut tools = self.list_tools(service_name).await?;
                tools.sort_by(|left, right| left.name.cmp(&right.name));
                tools
                    .into_iter()
                    .map(|tool| {
                        Self::scoped_tool_entry(
                            tool.name.clone(),
                            tool.name,
                            service_name.to_string(),
                            service_name.to_string(),
                            tool.description,
                            tool.input_schema,
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_tool_descriptions_scoped().await,
            (Some(agent_id), Some(service_name)) => {
                let global_service_name = self
                    .resolve_service_name_for_agent(agent_id, service_name)
                    .await?;
                let service = self
                    .find_service(&global_service_name)
                    .await
                    .ok_or_else(|| StoreError::ServiceNotFound(global_service_name.clone()))?;
                let mut tools = self.list_tools(&global_service_name).await?;
                tools.sort_by(|left, right| left.name.cmp(&right.name));
                tools
                    .into_iter()
                    .map(|tool| {
                        Self::scoped_tool_entry(
                            tool.name.clone(),
                            tool.name,
                            service.original_name.clone(),
                            global_service_name.clone(),
                            tool.description,
                            tool.input_schema,
                        )
                    })
                    .collect()
            }
            (Some(agent_id), None) => self.collect_agent_tool_descriptions_scoped(agent_id).await,
        }
    }

    pub async fn list_resources_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        match (agent_id, service_name) {
            (_, Some(service_name)) => {
                let (display_service_name, global_service_name) = self
                    .resolve_scoped_service_binding(agent_id, service_name)
                    .await?;
                let mut resources = self.list_resources(&global_service_name).await?;
                resources.sort_by(|left, right| {
                    Self::value_field(left, "uri").cmp(Self::value_field(right, "uri"))
                });
                resources
                    .into_iter()
                    .map(|resource| {
                        Self::resource_payload_value(
                            resource,
                            display_service_name.clone(),
                            global_service_name.clone(),
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_resources_scoped().await,
            (Some(agent_id), None) => self.collect_agent_resources_scoped(agent_id).await,
        }
    }

    pub async fn list_resource_templates_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        match (agent_id, service_name) {
            (_, Some(service_name)) => {
                let (display_service_name, global_service_name) = self
                    .resolve_scoped_service_binding(agent_id, service_name)
                    .await?;
                let mut templates = self.list_resource_templates(&global_service_name).await?;
                templates.sort_by(|left, right| {
                    Self::value_field(left, "uriTemplate")
                        .cmp(Self::value_field(right, "uriTemplate"))
                });
                templates
                    .into_iter()
                    .map(|template| {
                        Self::resource_template_payload_value(
                            template,
                            display_service_name.clone(),
                            global_service_name.clone(),
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_resource_templates_scoped().await,
            (Some(agent_id), None) => self.collect_agent_resource_templates_scoped(agent_id).await,
        }
    }

    pub async fn read_resource_scoped(
        &self,
        agent_id: Option<&str>,
        uri: &str,
        service_name: Option<&str>,
    ) -> Result<serde_json::Value> {
        let (_, global_service_name) = self
            .resolve_resource_service_binding(agent_id, uri, service_name)
            .await?;
        self.read_resource(&global_service_name, uri).await
    }

    pub async fn list_prompts_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        match (agent_id, service_name) {
            (_, Some(service_name)) => {
                let (display_service_name, global_service_name) = self
                    .resolve_scoped_service_binding(agent_id, service_name)
                    .await?;
                let mut prompts = self.list_prompts(&global_service_name).await?;
                prompts.sort_by(|left, right| {
                    Self::value_field(left, "name").cmp(Self::value_field(right, "name"))
                });
                prompts
                    .into_iter()
                    .map(|prompt| {
                        Self::prompt_payload_value(
                            prompt,
                            None,
                            display_service_name.clone(),
                            global_service_name.clone(),
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_prompts_scoped().await,
            (Some(agent_id), None) => self.collect_agent_prompts_scoped(agent_id).await,
        }
    }

    pub async fn get_prompt_scoped(
        &self,
        agent_id: Option<&str>,
        prompt_name: &str,
        arguments: serde_json::Value,
        service_name: Option<&str>,
    ) -> Result<serde_json::Value> {
        let (_, global_service_name, original_prompt_name) = self
            .resolve_prompt_binding(agent_id, prompt_name, service_name)
            .await?;
        self.get_prompt(&global_service_name, &original_prompt_name, arguments)
            .await
    }

    pub async fn service_status_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<serde_json::Value> {
        let status = self
            .service_status_entry_scoped(agent_id, service_name)
            .await?;
        serde_json::to_value(status).map_err(|error| {
            StoreError::Other(format!("Service status serialization failed: {error}"))
        })
    }

    pub async fn service_status_entry_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: &str,
    ) -> Result<ServiceStatus> {
        let global_service_name = match agent_id {
            None => service_name.to_string(),
            Some(agent_id) => {
                self.resolve_service_name_for_agent(agent_id, service_name)
                    .await?
            }
        };
        let status = self
            .cached_service_status(&global_service_name)
            .await?
            .unwrap_or(self.health_check(&global_service_name).await?);
        Ok(status)
    }

    pub async fn check_services_scoped(&self, agent_id: Option<&str>) -> Result<serde_json::Value> {
        let statuses = self.check_service_health_scoped(agent_id).await?;
        let mut result = serde_json::Map::new();
        for status in statuses {
            result.insert(status.service_name, serde_json::json!(status.health_status));
        }
        Ok(serde_json::Value::Object(result))
    }

    pub async fn check_service_health_scoped(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<ScopedServiceHealth>> {
        let services = self.list_service_entries_scoped(agent_id).await?;
        let mut statuses = Vec::with_capacity(services.len());
        for service in services {
            let service_name = service.service.name.clone();
            let global_service_name = service
                .global_name
                .as_deref()
                .unwrap_or(service.service.name.as_str());
            let status = self.health_check(global_service_name).await?;
            statuses.push(ScopedServiceHealth {
                service_name,
                health_status: status.health_status,
            });
        }
        Ok(statuses)
    }

    pub async fn wait_service_ready(&self, name: &str, timeout_secs: u64) -> Result<ServiceStatus> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let status = self.health_check(name).await?;
            if matches!(
                status.health_status,
                HealthStatus::Healthy | HealthStatus::Ready
            ) {
                return Ok(status);
            }
            if !self.pool.is_connected(name).await {
                match self.connect_service_internal(name, true).await {
                    Ok(()) => continue,
                    Err(_) if tokio::time::Instant::now() < deadline => {
                        tokio::time::sleep(Self::retry_poll_interval(&status)).await;
                        continue;
                    }
                    Err(_) => {}
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(StoreError::Other(format!(
                    "Wait for service ready timed out: {name} (status={}, error={})",
                    Self::health_status_name(&status.health_status),
                    status.current_error.unwrap_or_else(|| "none".to_string()),
                )));
            }
            tokio::time::sleep(Self::retry_poll_interval(&status)).await;
        }
    }
}
