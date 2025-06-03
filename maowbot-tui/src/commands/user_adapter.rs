// Adapter that uses common-ui commands and formats output for TUI
use maowbot_common_ui::{GrpcClient, commands::{user::{UserCommands, UserUpdates}, CommandError}};
use std::io::{stdin, stdout, Write};

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
            
            match UserCommands::create_user(client, username, true).await {
                Ok(result) => {
                    let user = result.data.user;
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
        
        "remove" => {
            if args.len() < 2 {
                return "Usage: user remove <usernameOrUUID>".to_string();
            }
            
            match UserCommands::delete_user(client, args[1], false).await {
                Ok(result) => {
                    format!("User {} has been removed.", result.data.user_id)
                }
                Err(e) => format!("Failed to remove user: {}", e),
            }
        }
        
        "edit" => {
            if args.len() < 2 {
                return "Usage: user edit <usernameOrUUID>".to_string();
            }
            
            // First get the user to show current state
            match UserCommands::get_user_info(client, args[1]).await {
                Ok(info) => {
                    let user = &info.data.user;
                    println!("Editing user: {} ({})", user.global_username, user.user_id);
                    println!("Current is_active: {}", user.is_active);
                    
                    print!("New is_active value (true/false): ");
                    stdout().flush().unwrap();
                    
                    let mut input = String::new();
                    stdin().read_line(&mut input).unwrap();
                    let new_active = input.trim().parse::<bool>().unwrap_or(user.is_active);
                    
                    let updates = UserUpdates {
                        is_active: Some(new_active),
                        username: None,
                    };
                    
                    match UserCommands::update_user(client, &user.user_id, updates).await {
                        Ok(result) => {
                            let updated = result.data.user;
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
                Err(e) => format!("Failed to get user: {}", e),
            }
        }
        
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <usernameOrUUID>".to_string();
            }
            
            match UserCommands::get_user_info(client, args[1]).await {
                Ok(result) => {
                    let user = &result.data.user;
                    let mut output = format!(
                        "User Information:\n  ID: {}\n  Username: {}\n  Active: {}\n  Created: {}\n  Last Seen: {}",
                        user.user_id,
                        user.global_username,
                        user.is_active,
                        user.created_at.as_ref().map(|t| format!("{}", t.seconds)).unwrap_or_default(),
                        user.last_seen.as_ref().map(|t| format!("{}", t.seconds)).unwrap_or_default()
                    );
                    
                    if !result.data.identities.is_empty() {
                        output.push_str("\n\nPlatform Identities:");
                        for identity in &result.data.identities {
                            output.push_str(&format!(
                                "\n  - {} ({}):\n    Display Name: {}",
                                identity.platform as i32,
                                identity.platform_user_id,
                                identity.platform_display_name
                            ));
                        }
                    }
                    
                    output
                }
                Err(e) => format!("Failed to get user info: {}", e),
            }
        }
        
        "search" => {
            if args.len() < 2 {
                return "Usage: user search <query>".to_string();
            }
            let query = args[1..].join(" ");
            
            match UserCommands::search_users(client, &query, 50).await {
                Ok(result) => {
                    if result.data.users.is_empty() {
                        "No users found matching the search criteria.".to_string()
                    } else {
                        let mut output = format!("Found {} users:\n", result.data.total_count);
                        for user in &result.data.users {
                            output.push_str(&format!(
                                "  {} - {} (Active: {})\n",
                                user.user_id,
                                user.global_username,
                                user.is_active
                            ));
                        }
                        output
                    }
                }
                Err(e) => format!("Search failed: {}", e),
            }
        }
        
        "list" => {
            let page_size = if args.len() >= 3 && args[1] == "p" {
                args[2].parse::<i32>().unwrap_or(20)
            } else {
                20
            };
            
            let mut output = String::new();
            let mut page_token = None;
            
            loop {
                match UserCommands::list_users(client, page_size, page_token.clone(), false).await {
                    Ok(result) => {
                        if output.is_empty() {
                            output.push_str(&format!("Total users: {}\n\n", result.data.total_count));
                        }
                        
                        for user in &result.data.users {
                            output.push_str(&format!(
                                "{} - {} (Active: {})\n",
                                user.user_id,
                                user.global_username,
                                user.is_active
                            ));
                        }
                        
                        if !result.data.has_more {
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
                        
                        page_token = Some(result.data.next_page_token);
                    }
                    Err(e) => {
                        output.push_str(&format!("Failed to list users: {}", e));
                        break;
                    }
                }
            }
            
            output
        }
        
        _ => format!("Unknown user subcommand: {}", args[0]),
    }
}