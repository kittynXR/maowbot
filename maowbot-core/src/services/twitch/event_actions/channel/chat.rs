use crate::Error;

pub async fn handle_chat_message() -> Result<(), Error> {
    // e.g. channel.chat.message
    Ok(())
}

pub async fn handle_chat_clear() -> Result<(), Error> {
    // channel.chat.clear
    Ok(())
}

pub async fn handle_chat_clear_user_messages() -> Result<(), Error> {
    // channel.chat.clear_user_messages
    Ok(())
}

pub async fn handle_chat_message_delete() -> Result<(), Error> {
    // channel.chat.message_delete
    Ok(())
}

pub async fn handle_chat_notification() -> Result<(), Error> {
    // channel.chat.notification
    Ok(())
}

pub async fn handle_chat_settings_update() -> Result<(), Error> {
    // channel.chat_settings.update
    Ok(())
}

pub async fn handle_chat_user_message_hold() -> Result<(), Error> {
    // channel.chat.user_message_hold
    Ok(())
}

pub async fn handle_chat_user_message_update() -> Result<(), Error> {
    // channel.chat.user_message_update
    Ok(())
}
