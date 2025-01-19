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

pub use db::Database;
pub use error::Error;
pub use http::{HttpClient, DefaultHttpClient};