use crate::store::prelude::*;

impl MCPStore {
    pub(in crate::agent) async fn scoped_service_bindings(
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

    pub(crate) async fn resolve_scoped_service_binding(
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

    pub(crate) async fn resolve_resource_service_binding(
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

    pub(crate) async fn resolve_prompt_binding(
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
}
