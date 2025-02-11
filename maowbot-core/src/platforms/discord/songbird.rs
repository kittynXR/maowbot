//! songbird.rs
//!
//! Eventually this will integrate Songbird to join voice channels, listen in,
//! and forward raw audio to some STT pipeline. For now, it's just a stub.

use crate::Error;

/// SongbirdManager is a placeholder for future voice integration.
/// In a real implementation, we'd store references to Discord's gateway or
/// a Songbird call object, etc.
pub struct SongbirdManager {
    // For now, empty
}

impl SongbirdManager {
    pub fn new() -> Self {
        Self {}
    }

    /// Stub for joining a voice channel via Songbird.
    pub async fn join_voice_channel(
        &self,
        _guild_id: u64,
        _channel_id: u64,
    ) -> Result<(), Error> {
        // In a future version, you'd call: songbird.join(...).await
        Ok(())
    }

    /// Stub for leaving a voice channel.
    pub async fn leave_voice_channel(
        &self,
        _guild_id: u64
    ) -> Result<(), Error> {
        // In the future, you'd do: songbird.leave(...).await
        Ok(())
    }

    /// Stub for capturing and streaming voice data to STT.
    /// For now, we do nothing.
    pub async fn capture_audio_and_forward_to_stt(&self) -> Result<(), Error> {
        // In a real version, you'd attach an audio receiver to Songbird
        // and stream PCM data somewhere for transcription.
        Ok(())
    }
}
