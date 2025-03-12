use crate::platforms::twitch_eventsub::events::ChannelBitsUse;
use crate::Error;

pub async fn handle_bits_use(_evt: ChannelBitsUse) -> Result<(), Error> {
    // channel.bits.use
    Ok(())
}
