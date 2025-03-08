use crate::platforms::twitch_eventsub::events::{
    ChannelSharedChatBegin, ChannelSharedChatUpdate, ChannelSharedChatEnd
};
use crate::Error;

pub async fn handle_shared_chat_begin(evt: ChannelSharedChatBegin) -> Result<(), Error> {
    // stub for channel.shared_chat.begin
    Ok(())
}

pub async fn handle_shared_chat_update(evt: ChannelSharedChatUpdate) -> Result<(), Error> {
    // stub for channel.shared_chat.update
    Ok(())
}

pub async fn handle_shared_chat_end(evt: ChannelSharedChatEnd) -> Result<(), Error> {
    // stub for channel.shared_chat.end
    Ok(())
}
