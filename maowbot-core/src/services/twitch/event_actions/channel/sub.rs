use crate::platforms::twitch_eventsub::events::{
    ChannelSharedChatBegin, ChannelSharedChatUpdate, ChannelSharedChatEnd
};
use crate::Error;

pub async fn handle_shared_chat_begin(_evt: ChannelSharedChatBegin) -> Result<(), Error> {
    // stub for channel.shared_chat.begin
    Ok(())
}

pub async fn handle_shared_chat_update(_evt: ChannelSharedChatUpdate) -> Result<(), Error> {
    // stub for channel.shared_chat.update
    Ok(())
}

pub async fn handle_shared_chat_end(_evt: ChannelSharedChatEnd) -> Result<(), Error> {
    // stub for channel.shared_chat.end
    Ok(())
}
