use crate::Error;
use crate::models::{Command, User};
use crate::services::command_service::CommandContext;
use tracing::info;

pub async fn handle_followage(
    cmd: &Command,
    ctx: &CommandContext<'_>, // <-- Add lifetime parameter
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    let target_channel = &ctx.channel;

    info!(
        "(Stub) Attempting to fetch follow date for user_id={} in channel='{}'",
        user.user_id, target_channel
    );

    Ok(format!(
        "{} has been following {} for 3 months 2 days.",
        user.global_username.as_deref().unwrap_or("<unknown>"),
        target_channel
    ))
}