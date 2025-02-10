// =============================================================================
// maowbot-tui/src/commands/user.rs
// =============================================================================

use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use maowbot_core::models::User;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::Error;

/// Handle top-level "user" commands: add/remove/edit/info/search.
pub fn handle_user_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: user <add|remove|edit|info|search> [username]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: user add <username>".to_string();
            }
            let username = args[1];
            user_add(username, bot_api)
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: user remove <username>".to_string();
            }
            let username = args[1];
            user_remove(username, bot_api)
        }
        "edit" => {
            if args.len() < 2 {
                return "Usage: user edit <username>".to_string();
            }
            let username = args[1];
            user_edit(username, bot_api)
        }
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <username>".to_string();
            }
            let username = args[1];
            user_info(username, bot_api)
        }
        "search" => {
            if args.len() < 2 {
                return "Usage: user search <query>".to_string();
            }
            let query = args[1];
            user_search(query, bot_api)
        }
        _ => "Usage: user <add|remove|edit|info|search> [args]".to_string(),
    }
}

// -----------------------------------------------------------------------------
// "user add" => create a user row in DB
// -----------------------------------------------------------------------------
fn user_add(display_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    use uuid::Uuid;
    let new_user_id = Uuid::new_v4();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    match rt.block_on(bot_api.create_user(new_user_id, display_name)) {
        Ok(_) => format!(
            "Created user with user_id={} (display_name='{}').",
            new_user_id, display_name
        ),
        Err(e) => format!("Error creating user '{}': {:?}", display_name, e),
    }
}



// -----------------------------------------------------------------------------
// "user remove" => delete the user row
// -----------------------------------------------------------------------------
fn user_remove(typed_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // 1) find user by name
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.find_user_by_name(typed_name)) {
        Ok(user) => {
            match rt.block_on(bot_api.remove_user(user.user_id)) {
                Ok(_) => format!("Removed user '{}'.", typed_name),
                Err(e) => format!("Error removing '{}': {:?}", typed_name, e),
            }
        }
        Err(e) => format!("User '{}' not found or DB error => {:?}", typed_name, e),
    }
}



// -----------------------------------------------------------------------------
// "user edit" => fetch user from DB and allow interactive changes
// -----------------------------------------------------------------------------
fn user_edit(typed_user_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let parsed_id = match uuid::Uuid::parse_str(typed_user_id) {
        Ok(u) => u,
        Err(e) => return format!("Error: not a valid UUID '{}': {e}", typed_user_id),
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let maybe_user = match rt.block_on(bot_api.get_user(parsed_id)) {
        Ok(u) => u,
        Err(e) => {
            return format!("Error fetching user '{}': {:?}", parsed_id, e);
        }
    };

    let user = match maybe_user {
        Some(u) => u,
        None => return format!("User '{}' not found.", parsed_id),
    };

    // Start an interactive editor
    println!("Editing user '{}':", user.user_id);
    println!("(Press ENTER to keep current values.)");
    println!("-------------------------------------");

    println!("User is_active={}, set new value? (y/n):", user.is_active);
    print!("> ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let trimmed = line.trim();
    let new_is_active = if trimmed.is_empty() {
        user.is_active
    } else {
        trimmed.eq_ignore_ascii_case("y")
    };

    let update_res = rt.block_on(bot_api.update_user_active(user.user_id, new_is_active));
    match update_res {
        Ok(_) => format!(
            "Updated user '{}': is_active={} (was {}).",
            user.user_id, new_is_active, user.is_active
        ),
        Err(e) => format!("Error updating user '{}': {:?}", user.user_id, e),
    }
}

// -----------------------------------------------------------------------------
// "user info" => fetch from DB and display
// -----------------------------------------------------------------------------
fn user_info(typed_user_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let parsed_id = match uuid::Uuid::parse_str(typed_user_id) {
        Ok(u) => u,
        Err(e) => return format!("Error: not a valid UUID '{}': {e}", typed_user_id),
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    match rt.block_on(bot_api.get_user(parsed_id)) {
        Ok(Some(u)) => format!(
            "user_id='{}', global_username='{:?}', created_at={}, last_seen={}, is_active={}",
            u.user_id, u.global_username, u.created_at, u.last_seen, u.is_active
        ),
        Ok(None) => format!("User '{}' not found.", parsed_id),
        Err(e) => format!("Error fetching user '{}': {:?}", parsed_id, e),
    }
}


// -----------------------------------------------------------------------------
// "user search" => partial match or custom query
// -----------------------------------------------------------------------------
fn user_search(query: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.search_users(query)) {
        Ok(list) => {
            if list.is_empty() {
                format!("No users matched '{}'.", query)
            } else {
                let mut out = String::new();
                out.push_str(&format!("Found {} user(s):\n", list.len()));
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