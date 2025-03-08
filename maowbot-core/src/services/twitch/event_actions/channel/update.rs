use crate::platforms::twitch_eventsub::events::ChannelUpdate;
use crate::Error;

pub async fn handle_channel_update(evt: ChannelUpdate) -> Result<(), Error> {
    // TODO: Your logic when "channel.update" fires
    // e.g. log the new category/title in DB, announce changes, etc.
    Ok(())
}
