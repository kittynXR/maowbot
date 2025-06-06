use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{
    autostart_service_server::AutostartService,
    ListAutostartEntriesRequest, ListAutostartEntriesResponse, AutostartEntry,
    SetAutostartRequest, SetAutostartResponse,
    IsAutostartEnabledRequest, IsAutostartEnabledResponse,
    RemoveAutostartRequest, RemoveAutostartResponse,
};
use maowbot_core::repositories::postgres::autostart::AutostartRepository;
use std::sync::Arc;
use tracing::{info, error};

pub struct AutostartServiceImpl {
    autostart_repo: Arc<dyn AutostartRepository + Send + Sync>,
}

impl AutostartServiceImpl {
    pub fn new(autostart_repo: Arc<dyn AutostartRepository + Send + Sync>) -> Self {
        Self { autostart_repo }
    }
}

#[tonic::async_trait]
impl AutostartService for AutostartServiceImpl {
    async fn list_autostart_entries(
        &self,
        request: Request<ListAutostartEntriesRequest>,
    ) -> Result<Response<ListAutostartEntriesResponse>, Status> {
        let req = request.into_inner();
        info!("Listing autostart entries (enabled_only: {})", req.enabled_only);
        
        let entries = if req.enabled_only {
            self.autostart_repo.get_enabled_entries().await
        } else {
            self.autostart_repo.get_all_entries().await
        }.map_err(|e| {
            error!("Failed to list autostart entries: {:?}", e);
            Status::internal(format!("Failed to list autostart entries: {}", e))
        })?;
        
        let proto_entries: Vec<AutostartEntry> = entries.into_iter()
            .map(|e| AutostartEntry {
                id: e.id,
                platform: e.platform,
                account_name: e.account_name,
                enabled: e.enabled,
                created_at: e.created_at.to_rfc3339(),
                updated_at: e.updated_at.to_rfc3339(),
            })
            .collect();
        
        Ok(Response::new(ListAutostartEntriesResponse {
            entries: proto_entries,
        }))
    }
    
    async fn set_autostart(
        &self,
        request: Request<SetAutostartRequest>,
    ) -> Result<Response<SetAutostartResponse>, Status> {
        let req = request.into_inner();
        info!("Setting autostart for {}/{} to {}", req.platform, req.account_name, req.enabled);
        
        match self.autostart_repo.set_autostart(&req.platform, &req.account_name, req.enabled).await {
            Ok(_) => {
                Ok(Response::new(SetAutostartResponse {
                    success: true,
                    message: format!("Autostart {} for {}/{}", 
                        if req.enabled { "enabled" } else { "disabled" },
                        req.platform, req.account_name
                    ),
                }))
            }
            Err(e) => {
                error!("Failed to set autostart: {:?}", e);
                Ok(Response::new(SetAutostartResponse {
                    success: false,
                    message: format!("Failed to set autostart: {}", e),
                }))
            }
        }
    }
    
    async fn is_autostart_enabled(
        &self,
        request: Request<IsAutostartEnabledRequest>,
    ) -> Result<Response<IsAutostartEnabledResponse>, Status> {
        let req = request.into_inner();
        
        let enabled = self.autostart_repo
            .is_autostart_enabled(&req.platform, &req.account_name)
            .await
            .map_err(|e| {
                error!("Failed to check autostart status: {:?}", e);
                Status::internal(format!("Failed to check autostart status: {}", e))
            })?;
        
        Ok(Response::new(IsAutostartEnabledResponse { enabled }))
    }
    
    async fn remove_autostart(
        &self,
        request: Request<RemoveAutostartRequest>,
    ) -> Result<Response<RemoveAutostartResponse>, Status> {
        let req = request.into_inner();
        info!("Removing autostart for {}/{}", req.platform, req.account_name);
        
        match self.autostart_repo.remove_autostart(&req.platform, &req.account_name).await {
            Ok(_) => {
                Ok(Response::new(RemoveAutostartResponse {
                    success: true,
                    message: format!("Autostart removed for {}/{}", req.platform, req.account_name),
                }))
            }
            Err(e) => {
                error!("Failed to remove autostart: {:?}", e);
                Ok(Response::new(RemoveAutostartResponse {
                    success: false,
                    message: format!("Failed to remove autostart: {}", e),
                }))
            }
        }
    }
}