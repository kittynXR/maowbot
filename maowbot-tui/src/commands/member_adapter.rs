// Member command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::member::MemberCommands};
use std::io::stdin;

pub async fn handle_member_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: member <info|chat|list|search|note|merge|roles>".to_string();
    }

    match args[0] {
        "info" => {
            if args.len() < 2 {
                return "Usage: member info <usernameOrUUID>".to_string();
            }
            member_info(args[1], client).await
        }
        "chat" => {
            if args.len() < 2 {
                return "Usage: member chat <usernameOrUUID> [numMessages] [platform] [channel] [p <pageNum>] [s <search>]".to_string();
            }
            // Note: Chat functionality would require a messages service which doesn't seem to exist
            // in the proto files. For now, return a placeholder message.
            "Chat message functionality not yet implemented in gRPC services.".to_string()
        }
        "list" => {
            member_list(&args[1..], client).await
        }
        "search" => {
            if args.len() < 2 {
                return "Usage: member search <query>".to_string();
            }
            let query = args[1];
            member_search(query, client).await
        }
        "note" => {
            if args.len() < 3 {
                return "Usage: member note <usernameOrUUID> <note text...>".to_string();
            }
            let identifier = args[1];
            let note_text = args[2..].join(" ");
            member_note(identifier, &note_text, client).await
        }
        "merge" => {
            member_merge(&args[1..], client).await
        }
        "roles" => {
            member_roles(&args[1..], client).await
        }
        _ => {
            format!(
                "Unknown member subcommand '{}'. Type 'help member' for details.",
                args[0]
            )
        }
    }
}

/// Display info for one user
async fn member_info(identifier: &str, client: &GrpcClient) -> String {
    match MemberCommands::get_user_info(client, identifier).await {
        Ok(info) => {
            let user = &info.user;
            let mut output = String::new();
            output.push_str(&format!(
                "user_id={}\nglobal_username={}\ncreated_at={:?}\nlast_seen={:?}\nis_active={}\n\n",
                user.user_id,
                user.global_username,
                user.created_at,
                user.last_seen,
                user.is_active
            ));

            // Show platform identities
            if info.identities.is_empty() {
                output.push_str("No platform identities found.\n\n");
            } else {
                output.push_str("-- platform_identities:\n");
                for pid in info.identities {
                    output.push_str(&format!(
                        " platform={} platform_user_id={} username={} display_name={} roles={:?}\n",
                        pid.platform,
                        pid.platform_user_id,
                        pid.platform_username,
                        pid.platform_display_name,
                        pid.platform_roles
                    ));
                }
                output.push_str("\n");
            }

            // Show user analysis
            if let Some(analysis) = info.analysis {
                output.push_str("-- user_analysis:\n");
                output.push_str(&format!(" spam_score={}\n", analysis.spam_score));
                output.push_str(&format!(" intelligibility_score={}\n", analysis.intelligibility_score));
                output.push_str(&format!(" quality_score={}\n", analysis.quality_score));
                output.push_str(&format!(" horni_score={}\n", analysis.horni_score));
                output.push_str(&format!(" ai_notes={}\n", analysis.ai_notes));
                output.push_str(&format!(" moderator_notes={}\n", analysis.moderator_notes));
            } else {
                output.push_str("No user_analysis found.\n");
            }

            output
        }
        Err(e) => format!("Error: {}", e),
    }
}

/// List all users with optional pagination
async fn member_list(args: &[&str], client: &GrpcClient) -> String {
    match MemberCommands::search_users(client, "").await {
        Ok(result) => {
            let all_users = result.users;
            
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
                        " user_id={} global_username={} is_active={}\n",
                        u.user_id,
                        u.global_username,
                        u.is_active
                    ));
                }
                return out;
            }

            // Paginated listing
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
                        " user_id={} global_username={} is_active={}\n",
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
        Err(e) => format!("Error listing members => {}", e),
    }
}

/// Search for users
async fn member_search(query: &str, client: &GrpcClient) -> String {
    match MemberCommands::search_users(client, query).await {
        Ok(result) => {
            if result.users.is_empty() {
                return format!("No members found matching '{}'.", query);
            }

            let mut out = format!("Found {} member(s) matching '{}':\n", result.users.len(), query);
            for u in result.users {
                out.push_str(&format!(
                    " user_id={} global_username={} is_active={}\n",
                    u.user_id, u.global_username, u.is_active
                ));
            }
            out
        }
        Err(e) => format!("Error searching => {}", e),
    }
}

/// Add moderator note
async fn member_note(identifier: &str, note_text: &str, client: &GrpcClient) -> String {
    match MemberCommands::add_moderator_note(client, identifier, note_text).await {
        Ok(_) => format!("Moderator note added for user '{}'", identifier),
        Err(e) => format!("Error updating note => {}", e),
    }
}

/// Handle merge command
async fn member_merge(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage:\n  member merge <uuid1> <uuid2> [g <newGlobalUsername>]\n  member merge <username> [g <newGlobalUsername>]".to_string();
    }

    let first_arg = args[0];
    
    // Check if it's a UUID
    if uuid::Uuid::parse_str(first_arg).is_ok() && args.len() >= 2 {
        // Classic usage: member merge <uuid1> <uuid2> ...
        let uuid1_str = first_arg;
        let uuid2_str = args[1];

        // See if optional "g <newGlobalUsername>"
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

        match MemberCommands::merge_users(client, uuid1_str, uuid2_str, new_global_name.as_deref()).await {
            Ok(_result) => {
                if let Some(ref name) = new_global_name {
                    format!("Merged users successfully with new name='{}'", name)
                } else {
                    "Merged users successfully".to_string()
                }
            }
            Err(e) => format!("Error merging => {}", e),
        }
    } else {
        // New usage: member merge <username> [g <newGlobalUsername>]
        let mut new_global_name: Option<String> = None;
        let mut idx = 1;
        while idx < args.len() {
            if args[idx].eq_ignore_ascii_case("g") {
                idx += 1;
                if idx < args.len() {
                    new_global_name = Some(args[idx].to_string());
                }
            }
            idx += 1;
        }

        let username = first_arg;
        match MemberCommands::merge_duplicates(client, username, new_global_name.as_deref()).await {
            Ok(result) => {
                if let Some(ref new_name) = new_global_name {
                    format!(
                        "Merged {} duplicates into user_id={} and set global_username='{}'.",
                        result.merged_count,
                        result.merged_user.user_id,
                        new_name
                    )
                } else {
                    format!(
                        "Merged {} duplicates into user_id={}.",
                        result.merged_count,
                        result.merged_user.user_id
                    )
                }
            }
            Err(e) => format!("Error => {}", e),
        }
    }
}

/// Handle roles command
async fn member_roles(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: member roles <userNameOrUUID> [add <platform> <rolename>] [remove <platform> <rolename>]".to_string();
    }

    let identifier = args[0];

    // If no extra args => show all platform roles
    if args.len() == 1 {
        match MemberCommands::get_user_info(client, identifier).await {
            Ok(info) => {
                let mut out = format!("User roles for user_id={}:\n", info.user.user_id);
                if info.identities.is_empty() {
                    out.push_str("  (no platform identities)\n");
                } else {
                    for pid in info.identities {
                        out.push_str(&format!(
                            "  Platform={}: platform_user_id={} roles={:?}\n",
                            pid.platform,
                            pid.platform_user_id,
                            pid.platform_roles
                        ));
                    }
                }
                out
            }
            Err(e) => format!("Error: {}", e),
        }
    } else if args.len() < 4 {
        "Usage: member roles <userNameOrUUID> [add <platform> <rolename>] [remove <platform> <rolename>]".to_string()
    } else {
        let subcmd = args[1].to_lowercase();
        let platform_str = args[2];
        let rolename = args[3];

        match subcmd.as_str() {
            "add" => {
                match MemberCommands::add_role(client, identifier, platform_str, rolename).await {
                    Ok(_) => format!("Added role '{}' on platform='{}' for user '{}'", rolename, platform_str, identifier),
                    Err(e) => format!("Error adding role => {}", e),
                }
            }
            "remove" => {
                match MemberCommands::remove_role(client, identifier, platform_str, rolename).await {
                    Ok(_) => format!("Removed role '{}' on platform='{}' for user '{}'", rolename, platform_str, identifier),
                    Err(e) => format!("Error removing role => {}", e),
                }
            }
            _ => {
                "Usage: member roles <userNameOrUUID> [add <platform> <rolename>] [remove <platform> <rolename>]".to_string()
            }
        }
    }
}