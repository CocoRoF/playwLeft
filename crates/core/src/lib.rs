//! # playwLeft Core
//!
//! Agent-first browser automation engine built in Rust.
//!
//! This crate provides the core functionality for controlling Chromium browsers
//! via the Chrome DevTools Protocol (CDP). It is designed primarily for AI agents
//! and programmatic use cases where structured data extraction matters more than
//! visual rendering.
//!
//! ## Architecture
//!
//! - **protocol** — CDP WebSocket transport, session management, message types
//! - **browser** — Browser process lifecycle, context isolation
//! - **page** — Page navigation, content extraction, JavaScript evaluation
//! - **element** — DOM element interaction, selectors, queries
//! - **network** — Request/response interception, cookies, headers

pub mod browser;
pub mod element;
pub mod error;
pub mod network;
pub mod page;
pub mod protocol;

pub use browser::{Browser, BrowserContext, BrowserType, LaunchOptions};
pub use error::{PlaywLeftError, Result};
pub use network::{Request, Response, Route};
pub use page::{Frame, Page};

/// The main entry point for playwLeft.
///
/// Provides access to browser types (currently Chromium only).
pub struct PlaywLeft {
    pub chromium: BrowserType,
}

impl PlaywLeft {
    /// Create a new PlaywLeft instance.
    pub fn new() -> Self {
        Self {
            chromium: BrowserType::chromium(),
        }
    }
}

impl Default for PlaywLeft {
    fn default() -> Self {
        Self::new()
    }
}
