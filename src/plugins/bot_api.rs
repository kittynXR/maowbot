// src/plugins/bot_api.rs

#[derive(Debug)]
pub struct StatusData {
    pub connected_plugins: Vec<String>,
    pub uptime_seconds: u64,
}

pub trait BotApi: Send + Sync {
    fn list_plugins(&self) -> Vec<String>;
    fn status(&self) -> StatusData;
    fn shutdown(&self);
}