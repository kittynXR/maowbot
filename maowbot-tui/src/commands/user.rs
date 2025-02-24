use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use uuid::Uuid;

use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::Error;

pub async fn handle_user_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: user <add|remove|edit|info|search|list> [options]".to_string();
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
        _ => "Usage: user <add|remove|edit|info|search|list> [options]".to_string(),
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

    match bot_api.get_user(user_id).await {
        Ok(Some(u)) => format!(
            "user_id='{}', global_username='{:?}', created_at={}, last_seen={}, is_active={}",
            u.user_id, u.global_username, u.created_at, u.last_seen, u.is_active
        ),
        Ok(None) => format!("User '{}' not found.", user_id),
        Err(e) => format!("Error fetching user '{}': {:?}", user_id, e),
    }
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
            return output;
        } else {
            return format!(
                "Done listing all {} user(s) in {} page(s).",
                total, total_pages
            );
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