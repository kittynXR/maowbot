// =============================================================================
// maowbot-tui/src/commands/user.rs
//   (New file - the 'user' command: add/remove/edit/info/search in the `users` table)
// =============================================================================

use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use maowbot_core::plugins::bot_api::BotApi;

/// In a real system, you'd likely have a direct user-service method, but here
/// we'll simulate by calling gRPC or a specialized interface. We can place
/// some stubs in BotApi for user logic if we like. For demonstration, we'll
/// assume you’ll expand BotApi to handle user CRUD calls (not shown in
/// your code snippet). For now we just mock them.

pub fn handle_user_command(args: &[&str], _bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: user <add|remove|edit|info|search> [username]".to_string();
    }
    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: user add <username>".to_string();
            }
            let username = args[1];
            // In real code, call an API to create user in DB:
            format!("(Stub) Adding user '{}'.", username)
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: user remove <username>".to_string();
            }
            let username = args[1];
            // In real code, check if this is owner user => disallow
            format!("(Stub) Removing user '{}'.", username)
        }
        "edit" => {
            if args.len() < 2 {
                return "Usage: user edit <username>".to_string();
            }
            let username = args[1];
            // In real code, fetch existing user, show current data:
            let current = Some(UserStub { username: username.to_string(), is_active: true });
            edit_user_interactive(current)
        }
        "info" => {
            if args.len() < 2 {
                return "Usage: user info <username>".to_string();
            }
            let username = args[1];
            // (Stub) fetch from DB:
            format!("(Stub) user info => username='{}', created_at=..., last_seen=...", username)
        }
        "search" => {
            if args.len() < 2 {
                return "Usage: user search <query>".to_string();
            }
            let query = args[1];
            // (Stub) search in DB:
            format!("(Stub) Searching users for '{}'. Found: ...", query)
        }
        _ => "Usage: user <add|remove|edit|info|search> [args]".to_string(),
    }
}

fn edit_user_interactive(existing: Option<UserStub>) -> String {
    match existing {
        Some(u) => {
            println!("Editing user '{}':", u.username);
            println!("Press ENTER to keep current values.");

            // is_active
            println!("Current is_active={}. Enter new (y/n):", u.is_active);
            print!("> ");
            let _ = stdout().flush();
            let mut line = String::new();
            let _ = stdin().read_line(&mut line);
            let trimmed = line.trim();
            let new_is_active = if trimmed.is_empty() {
                u.is_active
            } else {
                trimmed.eq_ignore_ascii_case("y")
            };
            // (Stub) we'd do something in DB to update user

            format!("(Stub) Updated user '{}' => is_active={}", u.username, new_is_active)
        }
        None => "User not found.".to_string(),
    }
}

#[derive(Clone)]
struct UserStub {
    username: String,
    is_active: bool,
}