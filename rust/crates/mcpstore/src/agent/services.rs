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
}
