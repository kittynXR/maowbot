// File: maowbot-core/src/services/builtin_commands/vrchat_commands.rs

use crate::Error;
use crate::models::{Command, User};
use crate::services::command_service::CommandContext;
use tracing::{info, warn};

pub async fn handle_world(
    cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    // Instead of ctx.responding_account_name(), use respond_credential_name.as_deref()
    let account_name = ctx
        .respond_credential_name
        .as_deref()
        .unwrap_or("default_vrchat_account");

    info!("(Stub) handle_world => account_name='{}'", account_name);

    let result_stub = "World: 'Furry World' by 'FurAuthor', capacity=32, created=2020-10-12, updated=2022-11-05";
    Ok(result_stub.to_string())
}

pub async fn handle_instance(
    cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    let account_name = ctx
        .respond_credential_name
        .as_deref()
        .unwrap_or("default_vrchat_account");
    info!("(Stub) handle_instance => account_name='{}'", account_name);

    let instance_url = "vrchat://launch?ref=somewhere&worldId=wrld_123&instanceId=1234~public";
    Ok(format!(
        "Currently in wrld_123:1234~public - join link: {}",
        instance_url
    ))
}

pub async fn handle_vrchat_online_offline(
    cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    raw_args: &str,
) -> Result<String, Error> {
    let arg = raw_args.trim().to_lowercase();
    match arg.as_str() {
        "offline" => Ok("VRChat commands now allowed offline or online (stub).".to_string()),
        "online" => Ok("VRChat commands now restricted to stream online only (stub).".to_string()),
        _ => {
            warn!("!vrchat unknown argument => '{}'", raw_args);
            Ok("Usage: !vrchat <offline|online>".to_string())
        }
    }
}