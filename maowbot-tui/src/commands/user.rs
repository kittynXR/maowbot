use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use uuid::Uuid;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::Error;
use tokio::runtime::Handle;

pub fn handle_user_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: user <add|remove|edit|info|search> [usernameOrUUID]".to_string();
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
                return "Usage: user edit <usernameOrUUID>".to_string();
            }
            let user_str = args[1];
            user_edit(user_str, bot_api)
        }
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <usernameOrUUID>".to_string();
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

fn user_add(display_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let new_user_id = Uuid::new_v4();
    let res = Handle::current().block_on(async {
        bot_api.create_user(new_user_id, display_name).await
    });
    match res {
        Ok(_) => format!(
            "Created user with user_id={} (display_name='{}').",
            new_user_id, display_name
        ),
        Err(e) => format!("Error creating user '{}': {:?}", display_name, e),
    }
}

fn user_remove(typed_name_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let parsed = Uuid::parse_str(typed_name_or_id);
    if let Ok(uuid_val) = parsed {
        // They passed a UUID
        let del_res = Handle::current().block_on(bot_api.remove_user(uuid_val));
        match del_res {
            Ok(_) => format!("Removed user by user_id='{}'.", uuid_val),
            Err(e) => format!("Error removing user_id='{}': {:?}", uuid_val, e),
        }
    } else {
        // They passed a username
        let found = Handle::current().block_on(bot_api.find_user_by_name(typed_name_or_id));
        match found {
            Ok(user) => {
                let del_res = Handle::current().block_on(bot_api.remove_user(user.user_id));
                match del_res {
                    Ok(_) => format!("Removed user '{}'.", typed_name_or_id),
                    Err(e) => format!("Error removing '{}': {:?}", typed_name_or_id, e),
                }
            }
            Err(e) => format!("User '{}' not found or DB error => {:?}", typed_name_or_id, e),
        }
    }
}

fn user_edit(typed_user_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // First try to parse as UUID
    let parsed_id = Uuid::parse_str(typed_user_or_id);
    let user_id = match parsed_id {
        Ok(uuid_val) => uuid_val,
        Err(_) => {
            // If not a UUID, treat it as a username
            let found = Handle::current().block_on(bot_api.find_user_by_name(typed_user_or_id));
            match found {
                Ok(u) => u.user_id,
                Err(e) => {
                    return format!("No user found with name '{}': {:?}", typed_user_or_id, e);
                }
            }
        }
    };

    // Fetch user by resolved UUID
    let maybe_user = Handle::current().block_on(bot_api.get_user(user_id));
    let user = match maybe_user {
        Ok(Some(u)) => u,
        Ok(None) => return format!("User '{}' not found.", user_id),
        Err(e) => return format!("Error fetching user '{}': {:?}", user_id, e),
    };

    println!("Editing user '{}':", user.user_id);
    println!("(Press ENTER to keep current values.)");
    println!("-------------------------------------");

    // Example: we only allow toggling is_active. Could expand as needed.
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

    let up_res = Handle::current().block_on(async {
        bot_api.update_user_active(user.user_id, new_is_active).await
    });
    match up_res {
        Ok(_) => format!(
            "Updated user '{}': is_active={} (was {}).",
            user.user_id, new_is_active, user.is_active
        ),
        Err(e) => format!("Error updating user '{}': {:?}", user.user_id, e),
    }
}

fn user_info(typed_user_or_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Attempt to parse as a UUID first
    let parsed_id = Uuid::parse_str(typed_user_or_id);
    let user_id = match parsed_id {
        Ok(uuid_val) => uuid_val,
        Err(_) => {
            // If not a UUID, treat as a username
            let found = Handle::current().block_on(bot_api.find_user_by_name(typed_user_or_id));
            match found {
                Ok(u) => u.user_id,
                Err(e) => {
                    return format!("No user found with name '{}': {:?}", typed_user_or_id, e);
                }
            }
        }
    };

    let found = Handle::current().block_on(bot_api.get_user(user_id));
    match found {
        Ok(Some(u)) => format!(
            "user_id='{}', global_username='{:?}', created_at={}, last_seen={}, is_active={}",
            u.user_id, u.global_username, u.created_at, u.last_seen, u.is_active
        ),
        Ok(None) => format!("User '{}' not found.", user_id),
        Err(e) => format!("Error fetching user '{}': {:?}", user_id, e),
    }
}

fn user_search(query: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let res = Handle::current().block_on(bot_api.search_users(query));
    match res {
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