// maowbot-tui/src/commands/user.rs
// =============================================================================
//   - Removes the local runtime creation and uses tui_block_on(...) instead.
// =============================================================================

use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use maowbot_core::plugins::bot_api::BotApi;
use crate::tui_module::tui_block_on;

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
                return "Usage: user remove <usernameOrUUID>".to_string();
            }
            let user_str = args[1];
            user_remove(user_str, bot_api)
        }
        "edit" => {
            if args.len() < 2 {
                return "Usage: user edit <UUID>".to_string();
            }
            let user_str = args[1];
            user_edit(user_str, bot_api)
        }
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <UUID>".to_string();
            }
            let user_str = args[1];
            user_info(user_str, bot_api)
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

    match tui_block_on(bot_api.create_user(new_user_id, display_name)) {
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
fn user_remove(typed_name_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    use uuid::Uuid;

    // 1) Attempt to parse as UUID
    let parsed = Uuid::parse_str(typed_name_or_id);

    if let Ok(uuid_val) = parsed {
        // remove by direct ID
        match tui_block_on(bot_api.remove_user(uuid_val)) {
            Ok(_) => format!("Removed user by user_id='{}'.", uuid_val),
            Err(e) => format!("Error removing user_id='{}': {:?}", uuid_val, e),
        }
    } else {
        // 2) treat as a username, then find user
        match tui_block_on(bot_api.find_user_by_name(typed_name_or_id)) {
            Ok(user) => {
                match tui_block_on(bot_api.remove_user(user.user_id)) {
                    Ok(_) => format!("Removed user '{}'.", typed_name_or_id),
                    Err(e) => format!("Error removing '{}': {:?}", typed_name_or_id, e),
                }
            }
            Err(e) => format!("User '{}' not found or DB error => {:?}", typed_name_or_id, e),
        }
    }
}

// -----------------------------------------------------------------------------
// "user edit" => fetch user from DB and allow interactive changes
// -----------------------------------------------------------------------------
fn user_edit(typed_user_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    use uuid::Uuid;
    let parsed_id = match Uuid::parse_str(typed_user_id) {
        Ok(u) => u,
        Err(e) => return format!("Error: not a valid UUID '{}': {e}", typed_user_id),
    };

    let maybe_user = match tui_block_on(bot_api.get_user(parsed_id)) {
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
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let trimmed = line.trim();
    let new_is_active = if trimmed.is_empty() {
        user.is_active
    } else {
        trimmed.eq_ignore_ascii_case("y")
    };

    let update_res = tui_block_on(bot_api.update_user_active(user.user_id, new_is_active));
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
    use uuid::Uuid;
    let parsed_id = match Uuid::parse_str(typed_user_id) {
        Ok(u) => u,
        Err(e) => return format!("Error: not a valid UUID '{}': {e}", typed_user_id),
    };

    match tui_block_on(bot_api.get_user(parsed_id)) {
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
    match tui_block_on(bot_api.search_users(query)) {
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