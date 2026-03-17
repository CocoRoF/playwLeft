//! Browser management — launching, connecting, and controlling browser instances.

pub mod context;
pub mod launcher;

mod browser_impl;

pub use browser_impl::Browser;
pub use context::BrowserContext;
pub use launcher::{BrowserType, LaunchOptions};
