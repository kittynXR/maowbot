// File: maowbot-core/src/services/builtin_commands/mod.rs
//! Defines built-in commands such as `!ping`, `!followage`, `!world`, `!instance`, etc.
//! Each command is implemented in its own file, but we expose a single `handle_builtin_command`
//! entry point that the CommandService can call.

pub mod ping_command;
pub mod followage_command;
pub mod vrchat_commands;

use uuid::Uuid;
use crate::Error;
use crate::models::{Command, User};
use crate::services::builtin_commands::{
    ping_command::handle_ping,
    followage_command::handle_followage,
    vrchat_commands::{handle_world, handle_instance, handle_vrchat_online_offline},
};
use crate::services::command_service::CommandContext;

pub async fn handle_builtin_command(
    cmd: &Command,
    ctx: &CommandContext<'_>, // <-- Add lifetime
    user: &User,
    raw_args: &str,
) -> Result<Option<String>, Error> {
    let cname = cmd.command_name.to_lowercase();

    if cname == "!ping" {
        let resp = handle_ping(cmd, ctx, user, raw_args).await?;
        return Ok(Some(resp));
    }
    else if cname == "!followage" {
        let resp = handle_followage(cmd, ctx, user, raw_args).await?;
        return Ok(Some(resp));
    }
    else if cname == "!world" {
        let resp = handle_world(cmd, ctx, user, raw_args).await?;
        return Ok(Some(resp));
    }
    else if cname == "!instance" {
        let resp = handle_instance(cmd, ctx, user, raw_args).await?;
        return Ok(Some(resp));
    }
    else if cname == "!vrchat" {
        let resp = handle_vrchat_online_offline(cmd, ctx, user, raw_args).await?;
        return Ok(Some(resp));
    }

    Ok(None)
}