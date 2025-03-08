use crate::platforms::twitch_eventsub::events::StreamOnline;
use crate::Error;

pub async fn handle_stream_online(evt: StreamOnline) -> Result<(), Error> {
    // TODO: your logic when stream.online fires
    Ok(())
}
