use std::io::{stdin, stdout, Write};
use std::sync::Arc;
use uuid::Uuid;

use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::models::User;
use maowbot_core::Error;

/// This handles all `member` subcommands:
///   member info <identifier>
///   member chat <identifier> ...
///   member list ...
///   member search <query>
///   member note <identifier> <text>
///   member merge <uuid1> <uuid2> ...
///   member roles <identifier> [add <platform> <rolename>] [remove <platform> <rolename>]
///
pub async fn handle_member_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: member <info|chat|list|search|note|merge|roles>".to_string();
    }

    match args[0] {
        "info" => {
            if args.len() < 2 {
                return "Usage: member info <usernameOrUUID>".to_string();
            }
            member_info(args[1], bot_api).await
        }
        "chat" => {
            if args.len() < 2 {
                return "Usage: member chat <usernameOrUUID> [numMessages] [platform] [channel] [p <pageNum>] [s <search>]".to_string();
            }
            member_chat(&args[1..], bot_api).await
        }
        "list" => {
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
        "merge" => {
            member_merge(&args[1..], bot_api).await
        }
        "roles" => {
            // usage:
            //   member roles <userNameOrUUID>
            //     => show all roles from each platform
            //   member roles <userNameOrUUID> add <platform> <rolename>
            //   member roles <userNameOrUUID> remove <platform> <rolename>
            member_roles(&args[1..], bot_api).await
        }
        _ => {
            format!(
                "Unknown member subcommand '{}'. Type 'help member' for details.",
                args[0]
            )
        }
    }
}

/// `member info <identifier>`
async fn member_info(identifier: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

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

/// `member chat <usernameOrUUID> [numMessages] [platform] [channel] [p <pageNum>] [s <search>]`
async fn member_chat(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    let identifier = args[0];

    let mut idx = 1;
    let mut limit: i64 = i64::MAX;
    if args.len() > 1 {
        if let Ok(n) = args[1].parse::<i64>() {
            limit = n;
            idx += 1;
        }
    }

    let mut platform_opt: Option<String> = None;
    let mut channel_opt: Option<String> = None;
    let mut search_opt: Option<String> = None;
    let mut page = 1i64;

    while idx < args.len() {
        let token = args[idx].to_lowercase();
        match token.as_str() {
            "p" => {
                idx += 1;
                if idx < args.len() {
                    if let Ok(pg) = args[idx].parse::<i64>() {
                        page = pg;
                    }
                }
            }
            "s" => {
                idx += 1;
                if idx < args.len() {
                    search_opt = Some(args[idx].to_string());
                }
            }
            _ => {
                if platform_opt.is_none() {
                    platform_opt = Some(args[idx].to_string());
                } else if channel_opt.is_none() {
                    channel_opt = Some(args[idx].to_string());
                }
            }
        }
        idx += 1;
    }

    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

    let offset = if limit == i64::MAX {
        0
    } else {
        limit.saturating_mul(page.saturating_sub(1))
    };

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
        "Showing messages for user='{:?}' with optional filters:\n",
        user.global_username
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
    if limit < i64::MAX {
        out.push_str(&format!("  limit={}, page={}\n", limit, page));
    } else {
        out.push_str("  limit=ALL\n");
    }
    out.push_str("\n");

    for (i, m) in messages.iter().enumerate() {
        let n = i as i64 + 1 + offset;
        out.push_str(&format!(
            "[{}] {} {} {}: {}\n",
            n, m.timestamp, m.platform, m.channel, m.message_text
        ));
    }

    out
}

/// `member list [p <pageSize>]`
async fn member_list(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    let all_users = match bot_api.search_users("").await {
        Ok(u) => u,
        Err(e) => return format!("Error listing members => {:?}", e),
    };

    if all_users.is_empty() {
        return "No members found.".to_string();
    }

    let mut paging = false;
    let mut page_size = 25usize;
    let mut idx = 0;
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

/// `member search <query>`
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

/// `member note <identifier> <note text...>`
async fn member_note(identifier: &str, note_text: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

    match bot_api.append_moderator_note(user.user_id, note_text).await {
        Ok(_) => format!("Moderator note updated for user_id={}", user.user_id),
        Err(e) => format!("Error updating note => {:?}", e),
    }
}

/// `member merge <uuid1> <uuid2> [g <newGlobalUsername>]`
async fn member_merge(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: member merge <uuid1> <uuid2> [g <newGlobalUsername>]".to_string();
    }
    let uuid1_str = args[0];
    let uuid2_str = args[1];

    let user1_id = match Uuid::parse_str(uuid1_str) {
        Ok(x) => x,
        Err(e) => return format!("Error parsing uuid1: {e}"),
    };
    let user2_id = match Uuid::parse_str(uuid2_str) {
        Ok(x) => x,
        Err(e) => return format!("Error parsing uuid2: {e}"),
    };

    let mut new_global_name: Option<String> = None;
    let mut idx = 2;
    while idx < args.len() {
        if args[idx].eq_ignore_ascii_case("g") {
            idx += 1;
            if idx < args.len() {
                new_global_name = Some(args[idx].to_string());
            }
        }
        idx += 1;
    }

    let res = bot_api.merge_users(user1_id, user2_id, new_global_name.as_deref()).await;
    match res {
        Ok(()) => {
            let maybe_new = if let Some(n) = &new_global_name {
                format!(" with new global username='{n}'")
            } else {
                "".to_string()
            };
            format!(
                "Successfully merged user={} into user={}{}.",
                uuid2_str, uuid1_str, maybe_new
            )
        }
        Err(e) => format!("Error merging users => {e:?}"),
    }
}

/// NEW SUBCOMMAND: `member roles <identifier>`
async fn member_roles(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: member roles <userNameOrUUID> [add <platform> <rolename>] [remove <platform> <rolename>]".to_string();
    }

    let identifier = args[0];
    let user = match resolve_user(identifier, bot_api).await {
        Ok(u) => u,
        Err(e) => return format!("Error: {:?}", e),
    };

    // If no extra args => list all roles
    if args.len() == 1 {
        let mut out = String::new();
        out.push_str(&format!("User roles for user_id={}:\n", user.user_id));
        match bot_api.get_platform_identities_for_user(user.user_id).await {
            Ok(idens) => {
                if idens.is_empty() {
                    out.push_str("  (no platform identities)\n");
                } else {
                    for pid in idens {
                        out.push_str(&format!(
                            "  Platform={:?}: roles={:?}\n",
                            pid.platform, pid.platform_roles
                        ));
                    }
                }
            }
            Err(e) => {
                out.push_str(&format!("Error fetching platform_identities => {:?}", e));
            }
        }
        return out;
    }

    // We have sub-subcommands: add or remove
    if args.len() < 4 {
        return "Usage: member roles <userNameOrUUID> [add <platform> <rolename>] [remove <platform> <rolename>]".to_string();
    }

    let subcmd = args[1].to_lowercase();
    let platform_str = args[2];
    let rolename = args[3];

    match subcmd.as_str() {
        "add" => {
            let res = bot_api.add_role_to_user_identity(user.user_id, platform_str, rolename).await;
            match res {
                Ok(_) => format!("Added role '{}' on platform='{}' for user={}", rolename, platform_str, user.user_id),
                Err(e) => format!("Error adding role => {:?}", e),
            }
        }
        "remove" => {
            let res = bot_api.remove_role_from_user_identity(user.user_id, platform_str, rolename).await;
            match res {
                Ok(_) => format!("Removed role '{}' on platform='{}' for user={}", rolename, platform_str, user.user_id),
                Err(e) => format!("Error removing role => {:?}", e),
            }
        }
        _ => {
            "Usage: member roles <userNameOrUUID> [add <platform> <rolename>] [remove <platform> <rolename>]".to_string()
        }
    }
}

/// Helper to resolve user by either name or UUID
async fn resolve_user(identifier: &str, bot_api: &Arc<dyn BotApi>) -> Result<User, Error> {
    match Uuid::parse_str(identifier) {
        Ok(uuid_val) => {
            if let Some(u) = bot_api.get_user(uuid_val).await? {
                Ok(u)
            } else {
                Err(Error::Database(sqlx::Error::RowNotFound))
            }
        }
        Err(_) => {
            bot_api.find_user_by_name(identifier).await
        }
    }
}
