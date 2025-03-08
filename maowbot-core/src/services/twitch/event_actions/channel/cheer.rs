use crate::platforms::twitch_eventsub::events::ChannelCheer;
use crate::Error;

pub async fn handle_cheer(evt: ChannelCheer) -> Result<(), Error> {
    // channel.cheer
    Ok(())
}
