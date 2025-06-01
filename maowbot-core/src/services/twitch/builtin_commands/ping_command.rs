use maowbot_common::models::Command;
use maowbot_common::models::user::User;
use crate::Error;
use crate::services::twitch::command_service::CommandContext;

pub async fn handle_ping(
    _cmd: &Command,
    _ctx: &CommandContext<'_>,
    _user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    Ok("pong".to_string())
}