use crate::Error;
use crate::models::User;
use crate::models::Command;
use crate::services::twitch::command_service::CommandContext;

pub async fn handle_ping(
    _cmd: &Command,
    _ctx: &CommandContext<'_>,
    _user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    Ok("pong".to_string())
}