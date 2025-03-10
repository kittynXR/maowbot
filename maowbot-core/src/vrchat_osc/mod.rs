// File: maowbot-core/src/vrchat_osc/mod.rs
//! The root module for VRChat OSC support, including:
//!   - Parsing VRChat's avatar config JSON (avatar/)
//!   - Basic toggle controls (toggles/)
//!   - Robotics/AI control stubs (robotics/)
//!   - OSCQuery support for enumerating endpoints (oscquery/)
//!   - The main runtime that listens for OSC on port 9001, sends/receives messages, etc.

pub mod manager;
pub mod runtime;

pub mod avatar;
pub mod toggles;
pub mod robotics;
pub mod oscquery;
