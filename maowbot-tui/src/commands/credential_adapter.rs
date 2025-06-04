// Credential command adapter for TUI
use maowbot_common_ui::GrpcClient;
use maowbot_proto::maowbot::services::{
    ListCredentialsRequest, RefreshCredentialRequest, RevokeCredentialRequest,
    GetCredentialHealthRequest, BatchRefreshCredentialsRequest,
    credential_service_client::CredentialServiceClient,
};
use maowbot_proto::maowbot::common::Platform;

pub async fn handle_credential_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: credential <list|refresh|revoke|health|batch-refresh> [options]".to_string();
    }

    match args[0] {
        "list" => {
            let platform = args.get(1).and_then(|p| parse_platform(p).ok());
            list_credentials(client, platform).await
        }
        
        "refresh" => {
            if args.len() < 2 {
                return "Usage: credential refresh <credential_id>".to_string();
            }
            refresh_credential(client, args[1]).await
        }
        
        "revoke" => {
            if args.len() < 2 {
                return "Usage: credential revoke <credential_id> [--platform-revoke]".to_string();
            }
            let revoke_at_platform = args.get(2).map(|a| *a == "--platform-revoke").unwrap_or(false);
            revoke_credential(client, args[1], revoke_at_platform).await
        }
        
        "health" => {
            let platform = args.get(1).and_then(|p| parse_platform(p).ok());
            get_credential_health(client, platform).await
        }
        
        "batch-refresh" => {
            if args.len() < 2 {
                return "Usage: credential batch-refresh <platform> [--force]".to_string();
            }
            let platform = match parse_platform(args[1]) {
                Ok(p) => p,
                Err(e) => return format!("Invalid platform: {}", e),
            };
            let force = args.get(2).map(|a| *a == "--force").unwrap_or(false);
            batch_refresh_credentials(client, platform, force).await
        }
        
        _ => format!("Unknown credential subcommand: {}", args[0]),
    }
}

async fn list_credentials(client: &GrpcClient, platform: Option<Platform>) -> String {
    let request = ListCredentialsRequest {
        platforms: platform.map(|p| vec![p as i32]).unwrap_or_default(),
        active_only: false,
        include_expired: true,
        page: None,
    };
    
    let mut cred_client = client.credential.clone();
    match cred_client.list_credentials(request).await {
        Ok(response) => {
            let creds = response.into_inner().credentials;
            if creds.is_empty() {
                "No credentials found.".to_string()
            } else {
                let mut output = format!("Found {} credential(s):\n", creds.len());
                for info in creds {
                    if let Some(cred) = info.credential {
                        let platform_name = format_platform(cred.platform);
                        let status = match info.status {
                            1 => "Active",
                            2 => "Expired",
                            3 => "Refresh Needed",
                            4 => "Revoked",
                            5 => "Error",
                            _ => "Unknown",
                        };
                        output.push_str(&format!(
                            "  [{:<8}] {} - {} ({}{}{})\n",
                            status,
                            platform_name,
                            cred.user_name,
                            if cred.is_bot { "Bot" } else { "User" },
                            if cred.is_broadcaster { ", Broadcaster" } else { "" },
                            if cred.is_teammate { ", Teammate" } else { "" }
                        ));
                        output.push_str(&format!("    ID: {}\n", cred.credential_id));
                    }
                }
                output
            }
        }
        Err(e) => format!("Error listing credentials: {}", e),
    }
}

async fn refresh_credential(client: &GrpcClient, credential_id: &str) -> String {
    let request = RefreshCredentialRequest {
        identifier: Some(maowbot_proto::maowbot::services::refresh_credential_request::Identifier::CredentialId(
            credential_id.to_string()
        )),
        force_refresh: false,
    };
    
    let mut cred_client = client.credential.clone();
    match cred_client.refresh_credential(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.was_refreshed {
                "Credential refreshed successfully.".to_string()
            } else {
                format!("Credential not refreshed: {}", resp.error_message)
            }
        }
        Err(e) => format!("Error refreshing credential: {}", e),
    }
}

async fn revoke_credential(client: &GrpcClient, credential_id: &str, revoke_at_platform: bool) -> String {
    let request = RevokeCredentialRequest {
        identifier: Some(maowbot_proto::maowbot::services::revoke_credential_request::Identifier::CredentialId(
            credential_id.to_string()
        )),
        revoke_at_platform,
    };
    
    let mut cred_client = client.credential.clone();
    match cred_client.revoke_credential(request).await {
        Ok(_) => "Credential revoked successfully.".to_string(),
        Err(e) => format!("Error revoking credential: {}", e),
    }
}

async fn get_credential_health(client: &GrpcClient, platform: Option<Platform>) -> String {
    let request = GetCredentialHealthRequest {
        platforms: platform.map(|p| vec![p as i32]).unwrap_or_default(),
    };
    
    let mut cred_client = client.credential.clone();
    match cred_client.get_credential_health(request).await {
        Ok(response) => {
            let health = response.into_inner();
            let mut output = String::new();
            
            if let Some(overall) = health.overall {
                output.push_str(&format!(
                    "Overall Health: {:.1}% ({}/{} platforms healthy)\n\n",
                    overall.health_score * 100.0,
                    overall.healthy_platforms,
                    overall.total_platforms
                ));
            }
            
            for platform_health in health.platform_health {
                let platform_name = format_platform(platform_health.platform);
                output.push_str(&format!("Platform: {}\n", platform_name));
                output.push_str(&format!("  Total Credentials: {}\n", platform_health.total_credentials));
                output.push_str(&format!("  Active: {}\n", platform_health.active_credentials));
                output.push_str(&format!("  Expired: {}\n", platform_health.expired_credentials));
                output.push_str(&format!("  Expiring Soon: {}\n", platform_health.expiring_soon));
                output.push_str("\n");
            }
            
            output
        }
        Err(e) => format!("Error getting credential health: {}", e),
    }
}

async fn batch_refresh_credentials(client: &GrpcClient, platform: Platform, force: bool) -> String {
    // First get all credentials for the platform
    let list_request = ListCredentialsRequest {
        platforms: vec![platform as i32],
        active_only: false,
        include_expired: true,
        page: None,
    };
    
    let mut cred_client = client.credential.clone();
    let credentials = match cred_client.list_credentials(list_request).await {
        Ok(resp) => resp.into_inner().credentials,
        Err(e) => return format!("Error listing credentials: {}", e),
    };
    
    let credential_ids: Vec<String> = credentials
        .into_iter()
        .filter_map(|info| info.credential.map(|c| c.credential_id))
        .collect();
    
    if credential_ids.is_empty() {
        return format!("No credentials found for platform {}", format_platform(platform as i32));
    }
    
    let request = BatchRefreshCredentialsRequest {
        credential_ids,
        force_refresh: force,
        continue_on_error: true,
    };
    
    match cred_client.batch_refresh_credentials(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            let mut output = format!(
                "Batch refresh completed: {} succeeded, {} failed\n",
                resp.success_count, resp.failure_count
            );
            
            for result in resp.results {
                if !result.success {
                    output.push_str(&format!(
                        "  Failed {}: {}\n",
                        result.credential_id, result.error_message
                    ));
                }
            }
            
            output
        }
        Err(e) => format!("Error batch refreshing credentials: {}", e),
    }
}

fn parse_platform(platform_str: &str) -> Result<Platform, String> {
    match platform_str.to_lowercase().as_str() {
        "twitch" | "twitch-helix" => Ok(Platform::TwitchHelix),
        "twitch-irc" => Ok(Platform::TwitchIrc),
        "twitch-eventsub" => Ok(Platform::TwitchEventsub),
        "discord" => Ok(Platform::Discord),
        "vrchat" => Ok(Platform::Vrchat),
        _ => Err(format!("Unknown platform '{}'", platform_str)),
    }
}

fn format_platform(platform: i32) -> &'static str {
    match Platform::try_from(platform) {
        Ok(Platform::TwitchHelix) => "Twitch",
        Ok(Platform::TwitchIrc) => "Twitch-IRC",
        Ok(Platform::TwitchEventsub) => "Twitch-EventSub",
        Ok(Platform::Discord) => "Discord",
        Ok(Platform::Vrchat) => "VRChat",
        _ => "Unknown",
    }
}