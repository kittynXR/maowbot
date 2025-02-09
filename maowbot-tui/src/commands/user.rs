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
fn user_add(username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Here we assume the user wants to store the same string in `user_id` and
    // maybe also in `User.global_username`.
    // In real code, you might generate a new UUID for user_id, or prompt for it, etc.
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.create_user(username, username)) {
        Ok(_) => format!("Created user_id='{}' (global_username='{}').", username, username),
        Err(e) => format!("Error creating user '{}': {:?}", username, e),
    }
}

// -----------------------------------------------------------------------------
// "user remove" => delete the user row
// -----------------------------------------------------------------------------
fn user_remove(username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Possibly you’d block removal if the user is your “owner” or if references exist, etc.
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.remove_user(username)) {
        Ok(_) => format!("Removed user '{}'.", username),
        Err(e) => match e {
            Error::Database(db_err) => format!("DB error removing '{}': {:?}", username, db_err),
            other => format!("Error removing '{}': {:?}", username, other),
        },
    }
}

// -----------------------------------------------------------------------------
// "user edit" => fetch user from DB and allow interactive changes
// -----------------------------------------------------------------------------
fn user_edit(username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let maybe_user = match rt.block_on(bot_api.get_user(username)) {
        Ok(u) => u,
        Err(e) => {
            return format!("Error fetching user '{}': {:?}", username, e);
        }
    };

    let user = match maybe_user {
        Some(u) => u,
        None => return format!("User '{}' not found.", username),
    };

    // Start an interactive editor
    println!("Editing user '{}':", user.user_id);
    println!("(Press ENTER to keep current values.)");
    println!("-------------------------------------");

    // Current is_active
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

    // Possibly also edit global_username here if you want:
    // e.g. prompt => "New username? (current: {user.global_username})"

    // Now do an update
    let update_res = rt.block_on(bot_api.update_user_active(&user.user_id, new_is_active));
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
fn user_info(username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.get_user(username)) {
        Ok(Some(u)) => format!(
            "user_id='{}', global_username='{:?}', created_at={}, last_seen={}, is_active={}",
            u.user_id,
            u.global_username,
            u.created_at,
            u.last_seen,
            u.is_active
        ),
        Ok(None) => format!("User '{}' not found.", username),
        Err(e) => format!("Error fetching user '{}': {:?}", username, e),
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