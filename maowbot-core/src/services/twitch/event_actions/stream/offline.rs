use crate::platforms::twitch_eventsub::events::StreamOffline;
use crate::Error;

pub async fn handle_stream_offline(evt: StreamOffline) -> Result<(), Error> {
    // TODO: your logic when stream.offline fires
    Ok(())
}
