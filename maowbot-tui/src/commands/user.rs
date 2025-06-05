// File: maowbot-tui/src/commands/user.rs (UPDATED)

use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use uuid::Uuid;
use maowbot_common::traits::api::BotApi;

pub async fn handle_user_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: user <add|remove|edit|info|search|list|find-duplicates|merge> [options]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: user add <username>".to_string();
            }
            let username = args[1];
            user_add(username, bot_api).await
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: user remove <usernameOrUUID>".to_string();
            }
            user_remove(args[1], bot_api).await
        }
        "edit" => {
            if args.len() < 2 {
                return "Usage: user edit <usernameOrUUID>".to_string();
            }
            user_edit(args[1], bot_api).await
        }
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <usernameOrUUID>".to_string();
            }
            user_info(args[1], bot_api).await
        }
        "search" => {
            if args.len() < 2 {
                return "Usage: user search <query>".to_string();
            }
            user_search(args[1], bot_api).await
        }
        "list" => {
            // Format: user list [p [num]]
            user_list(&args[1..], bot_api).await
        }
        "find-duplicates" => {
            user_find_duplicates(bot_api).await
        }
        "merge" => {
            if args.len() < 3 {
                return "Usage: user merge <primary-user-id> <duplicate-user-id> [<duplicate-user-id>...]".to_string();
            }
            user_merge(&args[1..], bot_api).await
        }
        _ => "Usage: user <add|remove|edit|info|search|list|find-duplicates|merge> [options]".to_string(),
    }
}

async fn user_add(display_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let new_user_id = Uuid::new_v4();
    match bot_api.create_user(new_user_id, display_name).await {
        Ok(_) => format!(
            "Created user with user_id={} (display_name='{}').",
            new_user_id, display_name
        ),
        Err(e) => format!("Error creating user '{}': {:?}", display_name, e),
    }
}

async fn user_remove(typed_name_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let parsed = Uuid::parse_str(typed_name_or_id);
    if let Ok(uuid_val) = parsed {
        // They passed a UUID
        match bot_api.remove_user(uuid_val).await {
            Ok(_) => format!("Removed user_id='{}'.", uuid_val),
            Err(e) => format!("Error removing user_id='{}': {:?}", uuid_val, e),
        }
    } else {
        // They passed a username
        match bot_api.find_user_by_name(typed_name_or_id).await {
            Ok(u) => {
                match bot_api.remove_user(u.user_id).await {
                    Ok(_) => format!("Removed user '{}'.", typed_name_or_id),
                    Err(e) => format!("Error removing '{}': {:?}", typed_name_or_id, e),
                }
            }
            Err(e) => format!("User '{}' not found or DB error => {:?}", typed_name_or_id, e),
        }
    }
}

async fn user_edit(typed_user_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user_id = match Uuid::parse_str(typed_user_or_id) {
        Ok(u) => u,
        Err(_) => {
            // If not a UUID, treat it as a username
            match bot_api.find_user_by_name(typed_user_or_id).await {
                Ok(u) => u.user_id,
                Err(e) => {
                    return format!("No user found with name '{}': {:?}", typed_user_or_id, e);
                }
            }
        }
    };

    let maybe_user = bot_api.get_user(user_id).await;
    let user = match maybe_user {
        Ok(Some(u)) => u,
        Ok(None) => return format!("User '{}' not found.", user_id),
        Err(e) => return format!("Error fetching user '{}': {:?}", user_id, e),
    };

    println!("Editing user '{}':", user.user_id);
    println!("(Press ENTER to keep current values.)");
    println!("-------------------------------------");

    // Example: only let them change is_active
    println!("User is_active={}, set new value? (y/n):", user.is_active);
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let trimmed = line.trim();
    let new_is_active = if trimmed.is_empty() {
        user.is_active
    } else {
        trimmed.eq_ignore_ascii_case("y")
    };

    match bot_api.update_user_active(user.user_id, new_is_active).await {
        Ok(_) => format!(
            "Updated user '{}': is_active={} (was {}).",
            user.user_id, new_is_active, user.is_active
        ),
        Err(e) => format!("Error updating user '{}': {:?}", user.user_id, e),
    }
}

async fn user_info(typed_user_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Resolve user ID
    let user_id = match Uuid::parse_str(typed_user_or_id) {
        Ok(u) => u,
        Err(_) => {
            match bot_api.find_user_by_name(typed_user_or_id).await {
                Ok(u) => u.user_id,
                Err(_) => {
                    return format!("No user found with name '{}'", typed_user_or_id);
                }
            }
        }
    };

    // Fetch user record
    let maybe_user = bot_api.get_user(user_id).await;
    let user = match maybe_user {
        Ok(Some(u)) => u,
        Ok(None) => return format!("User '{}' not found.", user_id),
        Err(e) => return format!("Error fetching user '{}': {:?}", user_id, e),
    };

    // Start building output
    let mut out = String::new();
    out.push_str(&format!("user_id={}\n", user.user_id));
    out.push_str(&format!(
        "global_username={}\n",
        user.global_username.as_deref().unwrap_or("(none)")
    ));
    out.push_str(&format!("created_at={}\n", user.created_at));
    out.push_str(&format!("last_seen={}\n", user.last_seen));
    out.push_str(&format!("is_active={}\n", user.is_active));

    // Now fetch all credentials belonging to this user
    let all_creds = match bot_api.list_credentials(None).await {
        Ok(list) => list,
        Err(e) => {
            out.push_str(&format!("Error listing user credentials => {e}\n"));
            return out;
        }
    };

    let user_creds: Vec<_> = all_creds.into_iter().filter(|c| c.user_id == user_id).collect();

    if user_creds.is_empty() {
        out.push_str("No accounts or platforms associated with this user.\n");
    } else {
        out.push_str("\n-- Accounts & Platforms --\n");
        for c in &user_creds {
            out.push_str(&format!("platform={:?}\n", c.platform));
            out.push_str(&format!("credential_id={}\n", c.credential_id));
            out.push_str(&format!("credential_type={:?}\n", c.credential_type));
            out.push_str(&format!("is_bot={}\n", c.is_bot));
            out.push_str(&format!("primary_token={}\n", c.primary_token));
            let refresh_str = c.refresh_token.as_deref().unwrap_or("(none)");
            out.push_str(&format!("refresh_token={}\n", refresh_str));
            out.push_str(&format!("expires_at={:?}\n", c.expires_at));
            out.push_str(&format!("created_at={}\n", c.created_at));
            out.push_str(&format!("updated_at={}\n", c.updated_at));
            out.push_str(&format!("additional_data={:?}\n", c.additional_data));
            out.push_str("\n");
        }
    }

    out
}

async fn user_search(query: &str, bot_api: &Arc<dyn BotApi>) -> String {
    match bot_api.search_users(query).await {
        Ok(list) => {
            if list.is_empty() {
                format!("No users matched '{}'.", query)
            } else {
                let mut out = format!("Found {} user(s):\n", list.len());
                for u in list {
                    out.push_str(&format!(
                        " - user_id='{}' username='{:?}' last_seen={}\n",
                        u.user_id, u.global_username, u.last_seen
                    ));
                }
                out
            }
        }
        Err(e) => format!("Error searching users for '{}': {:?}", query, e),
    }
}

/// Handles the "user list [p [num]]" logic:
/// - If "p" is provided, enable pagination with optional "num" as page size (default=25).
/// - If "p" not provided, simply list all users in one go.
async fn user_list(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    // We can attempt an "empty query" to fetch all users
    let all_users = match bot_api.search_users("").await {
        Ok(u) => u,
        Err(e) => return format!("Error listing users => {:?}", e),
    };

    if all_users.is_empty() {
        return "No users found in the database.".to_string();
    }

    // Check if "p" is the first argument => enable pagination
    if !args.is_empty() && args[0].eq_ignore_ascii_case("p") {
        // optional <num>
        let mut per_page = 25usize;
        if args.len() > 1 {
            if let Ok(n) = args[1].parse::<usize>() {
                per_page = n;
            }
        }

        let total = all_users.len();
        let mut output = String::new();
        let total_pages = (total + per_page - 1) / per_page;

        for (page_index, chunk) in all_users.chunks(per_page).enumerate() {
            output.push_str(&format!(
                "\n-- Page {}/{} (showing up to {} users) --\n",
                page_index + 1,
                total_pages,
                chunk.len()
            ));
            for u in chunk {
                output.push_str(&format!(
                    " - user_id='{}' global_username='{:?}' is_active={}\n",
                    u.user_id, u.global_username, u.is_active
                ));
            }

            // If this is not the last page, prompt to continue
            if page_index < total_pages - 1 {
                output.push_str("\nPress ENTER to continue...");
                println!("{}", output);
                output.clear();
                let mut line = String::new();
                let _ = stdin().read_line(&mut line);
            }
        }

        // After last page chunk, if there's anything left in "output", print it
        if !output.is_empty() {
            output
        } else {
            format!(
                "Done listing all {} user(s) in {} page(s).",
                total, total_pages
            )
        }
    } else {
        // Non-paginated listing
        let mut out = String::new();
        out.push_str(&format!("Listing {} user(s):\n", all_users.len()));
        for u in &all_users {
            out.push_str(&format!(
                " - user_id='{}' global_username='{:?}' is_active={}\n",
                u.user_id, u.global_username, u.is_active
            ));
        }
        out
    }
}

async fn user_find_duplicates(bot_api: &Arc<dyn BotApi>) -> String {
    // For now, we'll use a simple approach: get all users and group by normalized username
    let all_users = match bot_api.search_users("").await {
        Ok(users) => users,
        Err(e) => return format!("Error listing users: {:?}", e),
    };
    
    // Group users by lowercase username
    let mut groups: std::collections::HashMap<String, Vec<maowbot_common::models::user::User>> = std::collections::HashMap::new();
    
    for user in all_users {
        if let Some(username) = &user.global_username {
            let key = username.to_lowercase();
            groups.entry(key).or_insert_with(Vec::new).push(user);
        }
    }
    
    // Find groups with duplicates
    let mut duplicates: Vec<(String, Vec<maowbot_common::models::user::User>)> = groups
        .into_iter()
        .filter(|(_, users)| users.len() > 1)
        .collect();
    
    if duplicates.is_empty() {
        return "No duplicate users found.".to_string();
    }
    
    // Sort by username
    duplicates.sort_by(|a, b| a.0.cmp(&b.0));
    
    let mut out = format!("Found {} groups of duplicate users:\n\n", duplicates.len());
    
    for (username, users) in duplicates {
        out.push_str(&format!("Username: '{}' ({} duplicates)\n", username, users.len()));
        // Sort users by creation date to identify the oldest (primary) one
        let mut sorted_users = users;
        sorted_users.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        
        for (idx, user) in sorted_users.iter().enumerate() {
            let marker = if idx == 0 { " [OLDEST/PRIMARY]" } else { "" };
            out.push_str(&format!(
                "  {} - ID: {} | Created: {} | Last seen: {}{}\n",
                idx + 1,
                user.user_id,
                user.created_at.format("%Y-%m-%d %H:%M:%S"),
                user.last_seen.format("%Y-%m-%d %H:%M:%S"),
                marker
            ));
        }
        out.push_str("\n");
    }
    
    out.push_str("To merge duplicates, use: user merge <primary-user-id> <duplicate-user-id> [<duplicate-user-id>...]\n");
    out.push_str("Tip: The oldest user (marked as PRIMARY) is usually the best choice as the primary user.\n");
    
    out
}

async fn user_merge(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: user merge <primary-user-id> <duplicate-user-id> [<duplicate-user-id>...]".to_string();
    }
    
    // Parse primary user ID
    let primary_id = match Uuid::parse_str(args[0]) {
        Ok(id) => id,
        Err(_) => return format!("Invalid primary user ID: {}", args[0]),
    };
    
    // Parse duplicate user IDs
    let mut duplicate_ids = Vec::new();
    for i in 1..args.len() {
        match Uuid::parse_str(args[i]) {
            Ok(id) => duplicate_ids.push(id),
            Err(_) => return format!("Invalid duplicate user ID: {}", args[i]),
        }
    }
    
    // Verify all users exist
    let primary_user = match bot_api.get_user(primary_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return format!("Primary user not found: {}", primary_id),
        Err(e) => return format!("Error fetching primary user: {:?}", e),
    };
    
    let mut duplicate_users = Vec::new();
    for dup_id in &duplicate_ids {
        match bot_api.get_user(*dup_id).await {
            Ok(Some(user)) => duplicate_users.push(user),
            Ok(None) => return format!("Duplicate user not found: {}", dup_id),
            Err(e) => return format!("Error fetching duplicate user {}: {:?}", dup_id, e),
        }
    }
    
    // Show what will be merged
    let mut confirmation = format!("Merge operation summary:\n\n");
    confirmation.push_str(&format!("PRIMARY USER (will be kept):\n"));
    confirmation.push_str(&format!("  ID: {} | Username: {:?} | Created: {}\n\n", 
        primary_user.user_id, 
        primary_user.global_username,
        primary_user.created_at.format("%Y-%m-%d %H:%M:%S")
    ));
    
    confirmation.push_str("DUPLICATE USERS (will be merged and deleted):\n");
    for user in &duplicate_users {
        confirmation.push_str(&format!("  ID: {} | Username: {:?} | Created: {}\n", 
            user.user_id,
            user.global_username,
            user.created_at.format("%Y-%m-%d %H:%M:%S")
        ));
    }
    
    confirmation.push_str("\nThis will:\n");
    confirmation.push_str("- Move all platform identities from duplicates to primary user\n");
    confirmation.push_str("- Move all command/redeem usage history to primary user\n");
    confirmation.push_str("- Delete the duplicate user records\n");
    confirmation.push_str("\nProceed? (yes/no): ");
    
    print!("{}", confirmation);
    stdout().flush().unwrap();
    
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    
    if !input.trim().eq_ignore_ascii_case("yes") {
        return "Merge cancelled.".to_string();
    }
    
    // For now, return a message about manual database operations needed
    // In a real implementation, we would call a bot API method to perform the merge
    let mut sql_commands = String::new();
    sql_commands.push_str("-- SQL commands to merge users (run these manually in the database):\n");
    sql_commands.push_str("BEGIN;\n\n");
    
    for dup_id in &duplicate_ids {
        sql_commands.push_str(&format!("-- Merge user {} into {}\n", dup_id, primary_id));
        sql_commands.push_str(&format!("UPDATE platform_identities SET user_id = '{}' WHERE user_id = '{}';\n", primary_id, dup_id));
        sql_commands.push_str(&format!("UPDATE command_usage SET user_id = '{}' WHERE user_id = '{}';\n", primary_id, dup_id));
        sql_commands.push_str(&format!("UPDATE redeem_usage SET user_id = '{}' WHERE user_id = '{}';\n", primary_id, dup_id));
        sql_commands.push_str(&format!("DELETE FROM user_analysis WHERE user_id = '{}';\n", dup_id));
        sql_commands.push_str(&format!("UPDATE user_audit_logs SET user_id = '{}' WHERE user_id = '{}';\n", primary_id, dup_id));
        sql_commands.push_str(&format!("DELETE FROM users WHERE user_id = '{}';\n", dup_id));
        sql_commands.push_str("\n");
    }
    
    sql_commands.push_str("COMMIT;\n");
    sql_commands.push_str("\n-- Run these commands in your PostgreSQL database to complete the merge.\n");
    
    sql_commands
}
