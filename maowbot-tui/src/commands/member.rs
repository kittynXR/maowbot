use std::io::{stdin, stdout, Write};
use std::sync::Arc;
use uuid::Uuid;

use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::models::User;
use maowbot_core::Error;

/// Handle "member" subcommands:
///   member info <identifier>
///   member chat <n> [platform [channel]] [p [pageSize]] [s <search text>]
///   member list [p [pageSize]]
///   member search <query>
///   member note <identifier> <note text...>
pub async fn handle_member_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_member_usage();
    }

    match args[0] {
        "info" => {
            if args.len() < 2 {
                return "Usage: member info <usernameOrUUID>".to_string();
            }
            member_info(args[1], bot_api).await
        }
        "chat" => {
            // Format: member chat <n> [platform [channel]] [p <pageNumber>] [s <search text>]
            if args.len() < 2 {
                return "Usage: member chat <numMessages> [platform] [channel] [p <pageNum>] [s <search>]"
                    .to_string();
            }
            member_chat(&args[1..], bot_api).await
        }
        "list" => {
            // Format: member list [p [pageSize]]
            member_list(&args[1..], bot_api).await
        }
        "search" => {
            if args.len() < 2 {
                return "Usage: member search <query>".to_string();
            }
            let query = args[1];
            member_search(query, bot_api).await
        }
        "note" => {
            if args.len() < 3 {
                return "Usage: member note <usernameOrUUID> <note text...>".to_string();
            }
            let identifier = args[1];
            let note_text = args[2..].join(" ");
            member_note(identifier, &note_text, bot_api).await
        }
        _ => {
            format!(
                "Unknown member subcommand '{}'.\n{}",
                args[0],
                show_member_usage()
            )
        }
    }
}

/// Show usage for the `member` command group.
fn show_member_usage() -> String {
    r#"Member Command Usage:

  member info <usernameOrUUID>
     Shows detailed information about that member (user row, platform identities, user_analysis)

  member chat <n> [platform [channel]] [p <pageNumber>] [s <search text>]
     Displays up to 'n' messages from this member, with optional platform/channel filter, paging, and a text filter.

     Examples:
       member chat 20
       member chat 20 twitch #somechannel
       member chat 20 p 2
       member chat 10 s hello
       member chat 10 twitch #somechannel p 2 s "hello"

  member list [p [pageSize]]
     Lists all members (all users in the DB). Optionally paginated:
       member list
       member list p 25

  member search <query>
     Searches for members by partial match on name or user_id, etc.

  member note <usernameOrUUID> <note text...>
     Appends or updates a moderator note on that member’s record.
"#
        .to_string()
}

/// Implementation of `member info <identifier>`
async fn member_info(identifier: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Attempt to resolve to a user
    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

    // Show user row
    let mut output = String::new();
    output.push_str(&format!(
        "user_id={}\nglobal_username={:?}\ncreated_at={}\nlast_seen={}\nis_active={}\n\n",
        user.user_id,
        user.global_username,
        user.created_at,
        user.last_seen,
        user.is_active
    ));

    // Now fetch platform identities
    match bot_api.get_platform_identities_for_user(user.user_id).await {
        Ok(idens) => {
            if idens.is_empty() {
                output.push_str("No platform identities found.\n\n");
            } else {
                output.push_str("-- platform_identities:\n");
                for pid in idens {
                    output.push_str(&format!(
                        " platform={:?} username={} display_name={:?} roles={:?}\n",
                        pid.platform, pid.platform_username, pid.platform_display_name, pid.platform_roles
                    ));
                }
                output.push_str("\n");
            }
        }
        Err(e) => {
            output.push_str(&format!(
                "Error fetching platform_identities => {:?}\n\n",
                e
            ));
        }
    }

    // Fetch user_analysis
    match bot_api.get_user_analysis(user.user_id).await {
        Ok(Some(analysis)) => {
            output.push_str("-- user_analysis:\n");
            output.push_str(&format!(" spam_score={}\n", analysis.spam_score));
            output.push_str(&format!(" intelligibility_score={}\n", analysis.intelligibility_score));
            output.push_str(&format!(" quality_score={}\n", analysis.quality_score));
            output.push_str(&format!(" horni_score={}\n", analysis.horni_score));
            output.push_str(&format!(" ai_notes={:?}\n", analysis.ai_notes));
            output.push_str(&format!(" moderator_notes={:?}\n", analysis.moderator_notes));
        }
        Ok(None) => {
            output.push_str("No user_analysis found.\n");
        }
        Err(e) => {
            output.push_str(&format!("Error fetching user_analysis => {:?}\n", e));
        }
    }

    output
}

/// Implementation of `member chat <n> ...`
async fn member_chat(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    // The first argument is the "n" (number of messages)
    let n_str = args[0];
    let limit = match n_str.parse::<i64>() {
        Ok(x) => x,
        Err(_) => {
            return "Invalid number for <n> in 'member chat' command.".to_string();
        }
    };

    // Potentially parse other optional arguments:
    let mut platform_opt: Option<String> = None;
    let mut channel_opt: Option<String> = None;
    let mut search_opt: Option<String> = None;
    let mut page = 1i64; // default page is 1
    let mut idx = 1;

    while idx < args.len() {
        // Check if "p" or "s"
        let token = args[idx].to_lowercase();
        match token.as_str() {
            "p" => {
                // read next token as page or pageSize
                idx += 1;
                if idx < args.len() {
                    if let Ok(pg) = args[idx].parse::<i64>() {
                        page = pg;
                    }
                }
            }
            "s" => {
                // read next token(s) as search text
                idx += 1;
                if idx < args.len() {
                    // Could be quoted from the CLI, but we only get them separated by spaces here.
                    // We'll do a single token or re-join the rest if needed. For simplicity, just use next token.
                    // In practice, you'd parse carefully. We'll do a naive approach:
                    search_opt = Some(args[idx].to_string());
                }
            }
            _ => {
                // Could be platform or channel
                if platform_opt.is_none() {
                    platform_opt = Some(args[idx].to_string());
                } else if channel_opt.is_none() {
                    channel_opt = Some(args[idx].to_string());
                }
            }
        }
        idx += 1;
    }

    // We'll prompt for the user (identifier) now: we do not have a direct user param in the command syntax
    // This might be an oversight in the specification, but let's do an interactive approach:
    // Or we can clarify that the "identifier" wasn't part of the command?
    // The instructions mention "Displays the n most recent messages from this member" but didn't show
    // exactly how we parse. Possibly the user ID was omitted from the prompt.
    // We'll do an interactive question:
    println!("Enter the member's usernameOrUUID to fetch chat messages for:");
    print!("> ");
    let _ = stdout().flush();
    let mut input = String::new();
    if stdin().read_line(&mut input).is_err() {
        return "Error reading user identifier from stdin.".to_string();
    }
    let identifier = input.trim();
    if identifier.is_empty() {
        return "No user identifier provided. Aborting.".to_string();
    }

    // Resolve user
    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

    // Now compute offset from page
    // If "page" is 2 and limit=10 => offset= 10*(2-1)=10
    let offset = limit * (page - 1);

    // Fetch messages
    let result = bot_api
        .get_user_chat_messages(
            user.user_id,
            limit,
            offset,
            platform_opt.clone(),
            channel_opt.clone(),
            search_opt.clone(),
        )
        .await;

    let messages = match result {
        Ok(msgs) => msgs,
        Err(e) => return format!("Error fetching messages => {:?}", e),
    };

    if messages.is_empty() {
        return format!("No messages found (page={}, limit={}).", page, limit);
    }

    let mut out = String::new();
    out.push_str(&format!(
        "Showing up to {} messages for user='{:?}' (page={}), with optional filters:\n",
        messages.len(),
        user.global_username,
        page,
    ));
    if let Some(ref plat) = platform_opt {
        out.push_str(&format!("  platform='{}'\n", plat));
    }
    if let Some(ref chan) = channel_opt {
        out.push_str(&format!("  channel='{}'\n", chan));
    }
    if let Some(ref s) = search_opt {
        out.push_str(&format!("  search='{}'\n", s));
    }
    out.push_str("\n");

    for (i, m) in messages.iter().enumerate() {
        out.push_str(&format!(
            "[{}] {} {} {}: {}\n",
            i + 1 + offset as usize,
            m.timestamp,
            m.platform,
            m.channel,
            m.message_text
        ));
    }

    out
}

/// Implementation of `member list [p [pageSize]]`
async fn member_list(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    // We'll call the existing "search_users("")" logic to get all, then optionally do pagination:
    let all_users = match bot_api.search_users("").await {
        Ok(u) => u,
        Err(e) => return format!("Error listing members => {:?}", e),
    };

    if all_users.is_empty() {
        return "No members found.".to_string();
    }

    // Check for pagination
    let mut idx = 0;
    let mut paging = false;
    let mut page_size = 25usize;
    while idx < args.len() {
        if args[idx].eq_ignore_ascii_case("p") {
            paging = true;
            if idx + 1 < args.len() {
                if let Ok(num) = args[idx + 1].parse::<usize>() {
                    page_size = num;
                    idx += 1;
                }
            }
        }
        idx += 1;
    }

    if !paging {
        // Print all
        let mut out = format!("Listing {} members:\n", all_users.len());
        for u in &all_users {
            out.push_str(&format!(
                " user_id={} global_username={:?} is_active={}\n",
                u.user_id,
                u.global_username,
                u.is_active
            ));
        }
        return out;
    }

    // Paginated
    let total = all_users.len();
    let total_pages = (total + page_size - 1) / page_size;
    let mut out = String::new();

    let mut start = 0;
    let mut page_num = 1;
    while start < total {
        let end = std::cmp::min(start + page_size, total);
        out.push_str(&format!(
            "\n-- Page {}/{} ({} - {} of {}) --\n",
            page_num,
            total_pages,
            start + 1,
            end,
            total
        ));
        for u in &all_users[start..end] {
            out.push_str(&format!(
                " user_id={} global_username={:?} is_active={}\n",
                u.user_id,
                u.global_username,
                u.is_active
            ));
        }

        if page_num < total_pages {
            // prompt user to continue or press 'q' to quit
            out.push_str("\nPress ENTER to continue, or 'q' to stop listing...");
            println!("{}", out);
            out.clear();

            let mut line = String::new();
            let _ = stdin().read_line(&mut line);
            if line.trim().eq_ignore_ascii_case("q") {
                return "Listing aborted.".to_string();
            }
        }

        start += page_size;
        page_num += 1;
    }

    if !out.is_empty() {
        out
    } else {
        format!("\nDone listing {} members in {} pages.\n", total, total_pages)
    }
}

/// Implementation of `member search <query>`
async fn member_search(query: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let results = match bot_api.search_users(query).await {
        Ok(r) => r,
        Err(e) => return format!("Error searching => {:?}", e),
    };

    if results.is_empty() {
        return format!("No members found matching '{}'.", query);
    }

    let mut out = format!("Found {} member(s) matching '{}':\n", results.len(), query);
    for u in results {
        out.push_str(&format!(
            " user_id={} global_username={:?} is_active={}\n",
            u.user_id, u.global_username, u.is_active
        ));
    }
    out
}

/// Implementation of `member note <identifier> <note text...>`
async fn member_note(identifier: &str, note_text: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Resolve user
    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

    // Attempt to append or update the note
    match bot_api.append_moderator_note(user.user_id, note_text).await {
        Ok(_) => format!("Moderator note updated for user_id={}", user.user_id),
        Err(e) => format!("Error updating note => {:?}", e),
    }
}

/// Helper to resolve user by either name or UUID
async fn resolve_user(identifier: &str, bot_api: &Arc<dyn BotApi>) -> Result<User, Error> {
    match Uuid::parse_str(identifier) {
        Ok(uuid_val) => {
            // fetch by user_id
            if let Some(u) = bot_api.get_user(uuid_val).await? {
                Ok(u)
            } else {
                Err(Error::Database(sqlx::Error::RowNotFound))
            }
        }
        Err(_) => {
            // fetch by name
            bot_api.find_user_by_name(identifier).await
        }
    }
}