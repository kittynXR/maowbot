// File: maowbot-tui/src/commands/user_grpc.rs
// New implementation using gRPC instead of BotApi

use maowbot_common_ui::GrpcClient;
use maowbot_proto::maowbot::services::{
    CreateUserRequest, DeleteUserRequest, UpdateUserRequest, GetUserRequest,
    SearchUsersRequest, ListUsersRequest, GetPlatformIdentitiesRequest,
    SearchField, ListUsersFilter,
};
use maowbot_proto::maowbot::common::{PageRequest, User as ProtoUser};
// use maowbot_proto::prost_types::FieldMask;  // TODO: Fix prost_types import
use std::io::{stdin, stdout, Write};
use uuid::Uuid;

pub async fn handle_user_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: user <add|remove|edit|info|search|list> [options]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: user add <username>".to_string();
            }
            let username = args[1];
            user_add(username, client).await
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: user remove <usernameOrUUID>".to_string();
            }
            user_remove(args[1], client).await
        }
        "edit" => {
            if args.len() < 2 {
                return "Usage: user edit <usernameOrUUID>".to_string();
            }
            user_edit(args[1], client).await
        }
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <usernameOrUUID>".to_string();
            }
            user_info(args[1], client).await
        }
        "search" => {
            if args.len() < 2 {
                return "Usage: user search <query>".to_string();
            }
            let query = args[1..].join(" ");
            user_search(&query, client).await
        }
        "list" => {
            let page_size = if args.len() >= 3 && args[1] == "p" {
                args[2].parse::<i32>().unwrap_or(20)
            } else {
                20
            };
            user_list(page_size, client).await
        }
        _ => format!("Unknown user subcommand: {}", args[0]),
    }
}

async fn user_add(username: &str, client: &GrpcClient) -> String {
    let request = CreateUserRequest {
        user_id: String::new(), // Let server generate
        display_name: username.to_string(),
        is_active: true,
    };
    
    match client.user.clone().create_user(request).await {
        Ok(response) => {
            let user = response.into_inner().user.unwrap_or_default();
            format!(
                "User created successfully!\n  ID: {}\n  Username: {}\n  Active: {}",
                user.user_id,
                user.global_username,
                user.is_active
            )
        }
        Err(e) => format!("Failed to create user: {}", e),
    }
}

async fn user_remove(username_or_uuid: &str, client: &GrpcClient) -> String {
    // First, try to find the user
    let user_id = if let Ok(uuid) = Uuid::parse_str(username_or_uuid) {
        uuid.to_string()
    } else {
        // Look up by username
        match find_user_by_name(username_or_uuid, client).await {
            Some(user) => user.user_id,
            None => return format!("User not found: {}", username_or_uuid),
        }
    };
    
    let request = DeleteUserRequest {
        user_id: user_id.clone(),
        hard_delete: false, // Soft delete
    };
    
    match client.user.clone().delete_user(request).await {
        Ok(_) => format!("User {} has been removed.", username_or_uuid),
        Err(e) => format!("Failed to remove user: {}", e),
    }
}

async fn user_edit(username_or_uuid: &str, client: &GrpcClient) -> String {
    // First, get the user
    let user = match find_user_by_name_or_id(username_or_uuid, client).await {
        Some(u) => u,
        None => return format!("User not found: {}", username_or_uuid),
    };
    
    println!("Editing user: {} ({})", user.global_username, user.user_id);
    println!("Current is_active: {}", user.is_active);
    
    print!("New is_active value (true/false): ");
    stdout().flush().unwrap();
    
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    let new_active = input.trim().parse::<bool>().unwrap_or(user.is_active);
    
    let mut updated_user = user.clone();
    updated_user.is_active = new_active;
    
    let request = UpdateUserRequest {
        user_id: user.user_id,
        user: Some(updated_user),
        update_mask: None, // TODO: Fix FieldMask import
        // update_mask: Some(FieldMask {
        //     paths: vec!["is_active".to_string()],
        // }),
    };
    
    match client.user.clone().update_user(request).await {
        Ok(response) => {
            let updated = response.into_inner().user.unwrap_or_default();
            format!(
                "User updated successfully!\n  ID: {}\n  Username: {}\n  Active: {}",
                updated.user_id,
                updated.global_username,
                updated.is_active
            )
        }
        Err(e) => format!("Failed to update user: {}", e),
    }
}

async fn user_info(username_or_uuid: &str, client: &GrpcClient) -> String {
    let user = match find_user_by_name_or_id(username_or_uuid, client).await {
        Some(u) => u,
        None => return format!("User not found: {}", username_or_uuid),
    };
    
    let mut output = format!(
        "User Information:\n  ID: {}\n  Username: {}\n  Active: {}\n  Created: {}\n  Updated: {}",
        user.user_id,
        user.global_username,
        user.is_active,
        user.created_at.as_ref().map(|t| format!("{}", t.seconds)).unwrap_or_default(),
        user.last_seen.as_ref().map(|t| format!("{}", t.seconds)).unwrap_or_default()
    );
    
    // Get platform identities
    let identities_request = GetPlatformIdentitiesRequest {
        user_id: user.user_id.clone(),
        platforms: vec![], // All platforms
    };
    
    if let Ok(response) = client.user.clone().get_platform_identities(identities_request).await {
        let identities = response.into_inner().identities;
        if !identities.is_empty() {
            output.push_str("\n\nPlatform Identities:");
            for identity in identities {
                output.push_str(&format!(
                    "\n  - {} ({}):\n    Display Name: {}\n    Active: {}",
                    identity.platform as i32,
                    identity.platform_user_id,
                    identity.platform_display_name,
                    true // Platform identities don't have is_active field
                ));
            }
        }
    }
    
    // Note: Credentials are now managed separately via CredentialService
    // For now, we'll just show platform identities
    
    output
}

async fn user_search(query: &str, client: &GrpcClient) -> String {
    let request = SearchUsersRequest {
        query: query.to_string(),
        search_fields: vec![SearchField::Username as i32],
        page: Some(PageRequest {
            page_size: 50,
            page_token: String::new(),
        }),
    };
    
    match client.user.clone().search_users(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.results.is_empty() {
                "No users found matching the search criteria.".to_string()
            } else {
                let page_info = resp.page.unwrap_or_default();
                let mut output = format!("Found {} users:\n", page_info.total_count);
                for result in resp.results {
                    if let Some(user) = result.user {
                        output.push_str(&format!(
                            "  {} - {} (Active: {})\n",
                            user.user_id,
                            user.global_username,
                            user.is_active
                        ));
                    }
                }
                output
            }
        }
        Err(e) => format!("Search failed: {}", e),
    }
}

async fn user_list(page_size: i32, client: &GrpcClient) -> String {
    let mut output = String::new();
    let mut page_token = String::new();
    
    loop {
        let request = ListUsersRequest {
            page: Some(PageRequest {
                page_size,
                page_token: page_token.clone(),
            }),
            filter: Some(ListUsersFilter {
                active_only: false,
                platforms: vec![],
                roles: vec![],
            }),
            order_by: "created_at".to_string(),
            descending: false,
        };
        
        match client.user.clone().list_users(request).await {
            Ok(response) => {
                let resp = response.into_inner();
                let page_info = resp.page.unwrap_or_default();
                
                if output.is_empty() {
                    output.push_str(&format!("Total users: {}\n\n", page_info.total_count));
                }
                
                for user in resp.users {
                    output.push_str(&format!(
                        "{} - {} (Active: {})\n",
                        user.user_id,
                        user.global_username,
                        user.is_active
                    ));
                }
                
                if page_info.next_page_token.is_empty() {
                    break;
                }
                
                // Ask if user wants to continue
                print!("\nPress ENTER to continue or 'q' to quit: ");
                stdout().flush().unwrap();
                let mut input = String::new();
                stdin().read_line(&mut input).unwrap();
                
                if input.trim() == "q" {
                    break;
                }
                
                page_token = page_info.next_page_token;
            }
            Err(e) => {
                output.push_str(&format!("Failed to list users: {}", e));
                break;
            }
        }
    }
    
    output
}

// Helper functions
async fn find_user_by_name(username: &str, client: &GrpcClient) -> Option<ProtoUser> {
    let request = SearchUsersRequest {
        query: username.to_string(),
        search_fields: vec![SearchField::Username as i32],
        page: Some(PageRequest {
            page_size: 1,
            page_token: String::new(),
        }),
    };
    
    if let Ok(response) = client.user.clone().search_users(request).await {
        response.into_inner().results.into_iter()
            .filter_map(|r| r.user)
            .next()
    } else {
        None
    }
}

async fn find_user_by_name_or_id(username_or_uuid: &str, client: &GrpcClient) -> Option<ProtoUser> {
    if let Ok(uuid) = Uuid::parse_str(username_or_uuid) {
        let request = GetUserRequest {
            user_id: uuid.to_string(),
            include_identities: false,
            include_analysis: false,
        };
        
        if let Ok(response) = client.user.clone().get_user(request).await {
            return response.into_inner().user;
        }
    }
    
    find_user_by_name(username_or_uuid, client).await
}