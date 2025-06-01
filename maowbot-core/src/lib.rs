// src/lib.rs

pub mod db;
pub mod repositories;
pub mod platforms;
pub mod crypto;
pub mod auth;
pub mod http;
pub mod tasks;
pub mod plugins;
pub mod eventbus;
pub mod cache;
pub mod services;
pub mod test_utils;

pub use db::Database;
pub use maowbot_common::error::Error;
pub use http::{DefaultHttpClient, HttpClient};