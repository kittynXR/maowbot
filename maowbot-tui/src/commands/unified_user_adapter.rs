// Unified user command adapter for TUI - combines user and member functionality
use maowbot_common_ui::{GrpcClient, commands::{user::{UserCommands, UserUpdates}, member::MemberCommands}};
use std::io::{stdin, stdout, Write};

pub async fn handle_user_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: user <subcommand> [options]\n\nSubcommands:\n  \
                Basic Operations:\n    \
                add, remove, edit, info, list, search\n  \
                Extended Operations:\n    \
                chat, note, merge, roles, analysis".to_string();
    }

    match args[0] {
        // Basic user operations
        "add" | "create" => {
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
        
        "remove" | "delete" => {
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
        
        "edit" | "update" => {
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
                        global_username: None,
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
                Err(e) => format!("Failed to get user for editing: {}", e),
            }
        }
        
        "info" | "show" => {
            if args.len() < 2 {
                return "Usage: user info <usernameOrUUID>".to_string();
            }
            
            // Use member info for more detailed information
            match MemberCommands::get_member_info(client, args[1]).await {
                Ok(result) => {
                    let mut output = String::new();
                    
                    // Basic user info
                    let user = &result.user;
                    output.push_str(&format!("User Information:\n"));
                    output.push_str(&format!("  ID: {}\n", user.user_id));
                    output.push_str(&format!("  Username: {}\n", user.global_username));
                    output.push_str(&format!("  Active: {}\n", user.is_active));
                    output.push_str(&format!("  Created: {}\n", user.created_at));
                    output.push_str(&format!("  Updated: {}\n", user.updated_at));
                    
                    // Platform identities
                    if !result.identities.is_empty() {
                        output.push_str("\nPlatform Identities:\n");
                        for identity in &result.identities {
                            output.push_str(&format!("  {} - {} ({})\n", 
                                identity.platform, 
                                identity.platform_display_name.as_ref().unwrap_or(&identity.platform_username),
                                identity.platform_user_id
                            ));
                        }
                    }
                    
                    // User analysis
                    if let Some(analysis) = &result.analysis {
                        output.push_str(&format!("\nAnalysis:\n"));
                        output.push_str(&format!("  Total Messages: {}\n", analysis.total_messages));
                        output.push_str(&format!("  Commands Used: {}\n", analysis.commands_used));
                        output.push_str(&format!("  Last Active: {}\n", analysis.last_active_at));
                        if let Some(note) = &analysis.note {
                            output.push_str(&format!("  Note: {}\n", note));
                        }
                    }
                    
                    output
                }
                Err(e) => format!("Error getting user info: {}", e),
            }
        }
        
        "list" => {
            let page_size = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
            let page = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            
            match UserCommands::list_users(client, page_size, page).await {
                Ok(result) => {
                    if result.data.users.is_empty() {
                        "No users found.".to_string()
                    } else {
                        let mut output = format!("Users (page {}, {} per page):\n", page, page_size);
                        for user in result.data.users {
                            output.push_str(&format!(
                                "  {} - {} [{}]\n",
                                user.user_id,
                                user.global_username,
                                if user.is_active { "Active" } else { "Inactive" }
                            ));
                        }
                        output
                    }
                }
                Err(e) => format!("Error listing users: {}", e),
            }
        }
        
        "search" => {
            if args.len() < 2 {
                return "Usage: user search <query>".to_string();
            }
            let query = args[1];
            
            match UserCommands::search_users(client, query).await {
                Ok(result) => {
                    if result.data.results.is_empty() {
                        format!("No users found matching '{}'", query)
                    } else {
                        let mut output = format!("Found {} users:\n", result.data.results.len());
                        for user in result.data.results {
                            output.push_str(&format!(
                                "  {} - {} [{}]\n",
                                user.user_id,
                                user.global_username,
                                if user.is_active { "Active" } else { "Inactive" }
                            ));
                        }
                        output
                    }
                }
                Err(e) => format!("Error searching users: {}", e),
            }
        }
        
        // Extended member operations
        "chat" => {
            if args.len() < 2 {
                return "Usage: user chat <usernameOrUUID> [numMessages] [platform] [channel] [p <pageNum>] [s <search>]".to_string();
            }
            "Chat message functionality not yet implemented in gRPC services.".to_string()
        }
        
        "note" => {
            if args.len() < 3 {
                return "Usage: user note <usernameOrUUID> <note text...>".to_string();
            }
            let identifier = args[1];
            let note_text = args[2..].join(" ");
            
            match MemberCommands::update_user_note(client, identifier, &note_text).await {
                Ok(_) => format!("Note updated for user '{}'", identifier),
                Err(e) => format!("Error updating note: {}", e),
            }
        }
        
        "merge" => {
            if args.len() < 3 {
                return "Usage: user merge <primaryUsernameOrUUID> <secondaryUsernameOrUUID>".to_string();
            }
            let primary = args[1];
            let secondary = args[2];
            
            match MemberCommands::merge_users(client, primary, secondary).await {
                Ok(_) => format!("Successfully merged '{}' into '{}'", secondary, primary),
                Err(e) => format!("Error merging users: {}", e),
            }
        }
        
        "roles" => {
            if args.len() < 3 {
                return "Usage: user roles <add|remove|list> <username> [role]".to_string();
            }
            
            let action = args[1];
            let username = args[2];
            
            match action {
                "add" => {
                    if args.len() < 4 {
                        return "Usage: user roles add <username> <role>".to_string();
                    }
                    let role = args[3];
                    match MemberCommands::add_user_role(client, username, role).await {
                        Ok(_) => format!("Added role '{}' to user '{}'", role, username),
                        Err(e) => format!("Error adding role: {}", e),
                    }
                }
                "remove" => {
                    if args.len() < 4 {
                        return "Usage: user roles remove <username> <role>".to_string();
                    }
                    let role = args[3];
                    match MemberCommands::remove_user_role(client, username, role).await {
                        Ok(_) => format!("Removed role '{}' from user '{}'", role, username),
                        Err(e) => format!("Error removing role: {}", e),
                    }
                }
                "list" => {
                    match MemberCommands::list_user_roles(client, username).await {
                        Ok(result) => {
                            if result.roles.is_empty() {
                                format!("User '{}' has no roles", username)
                            } else {
                                let mut output = format!("Roles for user '{}':\n", username);
                                for role in result.roles {
                                    output.push_str(&format!("  - {}\n", role));
                                }
                                output
                            }
                        }
                        Err(e) => format!("Error listing roles: {}", e),
                    }
                }
                _ => "Usage: user roles <add|remove|list> <username> [role]".to_string(),
            }
        }
        
        "analysis" => {
            if args.len() < 2 {
                return "Usage: user analysis <usernameOrUUID>".to_string();
            }
            
            match MemberCommands::get_user_analysis(client, args[1]).await {
                Ok(result) => {
                    let analysis = &result.analysis;
                    let mut output = format!("User Analysis for '{}':\n", args[1]);
                    output.push_str(&format!("  Total Messages: {}\n", analysis.total_messages));
                    output.push_str(&format!("  Commands Used: {}\n", analysis.commands_used));
                    output.push_str(&format!("  Redeems Used: {}\n", analysis.redeems_used));
                    output.push_str(&format!("  First Seen: {}\n", analysis.first_seen_at));
                    output.push_str(&format!("  Last Active: {}\n", analysis.last_active_at));
                    output.push_str(&format!("  Average Messages/Day: {:.2}\n", analysis.average_messages_per_day));
                    output.push_str(&format!("  Favorite Platform: {}\n", analysis.favorite_platform));
                    if let Some(note) = &analysis.note {
                        output.push_str(&format!("  Note: {}\n", note));
                    }
                    output
                }
                Err(e) => format!("Error getting user analysis: {}", e),
            }
        }
        
        _ => {
            format!("Unknown user subcommand: {}\n\nUse 'help user' for available commands.", args[0])
        }
    }
}