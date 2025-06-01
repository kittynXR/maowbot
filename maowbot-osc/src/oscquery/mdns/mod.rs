//! A fully in-Rust mDNS server and responder, replicating the C# approach from VRCFaceTracking.
//!
//! We provide a `MdnsService` that binds to `224.0.0.251:5353`, listens for mDNS queries,
//! and sends out the required responses (PTR, SRV, A, TXT, etc.) to advertise
//! our OSC and OSCQuery endpoints.

pub mod packet;
pub mod records;
pub mod dns_reader;
pub mod dns_writer;
pub mod service;

pub use service::{MdnsService, AdvertisedService};
