// src/lib.rs

pub mod models;
pub mod db;
pub mod repositories;
pub mod platforms;
pub mod error;
pub mod crypto;
pub mod auth;
pub mod http;
pub mod tasks;
pub mod plugins;
pub mod eventbus;
pub mod cache;
pub mod services;
pub mod test_utils;
mod vrchat_osc;

pub use db::Database;
pub use error::Error;
pub use http::{HttpClient, DefaultHttpClient};