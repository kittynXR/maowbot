//! Implementation of a pure-Rust OSCQuery approach, replicating the
//! functionality from Oyasumi's library without any .NET sidecar.
pub mod client;
pub mod server;
pub mod models;
pub mod mdns;

pub use client::OscQueryClient;
pub use server::OscQueryServer;
