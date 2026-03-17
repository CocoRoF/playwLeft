//! Chrome DevTools Protocol (CDP) implementation.
//!
//! This module handles all communication with Chromium browsers:
//! - WebSocket transport for sending/receiving CDP messages
//! - Session management for targeting specific browser contexts and pages
//! - CDP message types and serialization

pub mod session;
pub mod transport;
pub mod types;

pub use session::CdpSession;
pub use transport::Transport;
pub use types::*;
