//! maowbot-osc/src/oscquery/mod.rs
//!
//! Implementation or stubs for OSCQuery (advertising, discovery, introspection).
//! VRChat's OSCQuery reference:
//! https://docs.vrchat.com/docs/oscquery

pub mod server;
pub mod discovery;

// Re-export items for convenience
pub use server::OscQueryServer;
pub use discovery::OscQueryDiscovery;
