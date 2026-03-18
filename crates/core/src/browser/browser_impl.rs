//! Browser instance management.
//!
//! Represents a running browser process and manages its contexts and pages.

use crate::browser::context::BrowserContext;
use crate::error::{PlaywLeftError, Result};
use crate::protocol::session::CdpSession;
use crate::protocol::transport::Transport;
use crate::protocol::types::{BrowserVersion, TargetInfo, Viewport};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// A running browser instance.
///
/// Manages the browser process lifecycle, provides methods for creating
/// isolated browser contexts, and handles the root CDP session.
pub struct Browser {
    /// Root CDP session for browser-level commands.
    session: CdpSession,
    /// Browser process handle (if we launched it).
    process: Option<Mutex<Child>>,
    /// Temporary directory for user data (cleaned up on drop).
    _temp_dir: Option<TempDir>,
    /// Default viewport for new contexts.
    default_viewport: Option<Viewport>,
    /// Cached browser version info.
    version: BrowserVersion,
}

impl Browser {
    /// Create a new Browser from an established transport connection.
    pub(crate) async fn new(
        transport: Arc<Transport>,
        process: Option<Child>,
        temp_dir: Option<TempDir>,
        default_viewport: Option<Viewport>,
    ) -> Result<Self> {
        let session = CdpSession::new_root(transport);

        // Enable target discovery
        session
            .send(
                "Target.setDiscoverTargets",
                serde_json::json!({ "discover": true }),
            )
            .await?;

        // Get browser version
        let version_result = session.send_simple("Browser.getVersion").await?;
        let version: BrowserVersion = serde_json::from_value(version_result)?;

        info!(
            product = %version.product,
            proto = %version.protocol_version,
            "Connected to browser"
        );

        Ok(Self {
            session,
            process: process.map(Mutex::new),
            _temp_dir: temp_dir,
            default_viewport,
            version,
        })
    }

    /// Create a new isolated browser context (like incognito mode).
    pub async fn new_context(&self) -> Result<BrowserContext> {
        let result = self
            .session
            .send(
                "Target.createBrowserContext",
                serde_json::json!({
                    "disposeOnDetach": true,
                }),
            )
            .await?;

        let context_id = result["browserContextId"]
            .as_str()
            .ok_or_else(|| PlaywLeftError::ProtocolError {
                code: -1,
                message: "No browserContextId in response".to_string(),
            })?
            .to_string();

        debug!(context_id = %context_id, "Created new browser context");

        BrowserContext::new(
            self.session.clone(),
            context_id,
            self.default_viewport.clone(),
        )
        .await
    }

    /// Create a new page in the default browser context.
    ///
    /// This is a convenience method. For isolation, prefer `new_context()`.
    pub async fn new_page(&self) -> Result<crate::page::Page> {
        let context = self.new_context().await?;
        context.new_page().await
    }

    /// Get all current browser contexts.
    pub async fn contexts(&self) -> Result<Vec<String>> {
        let result = self
            .session
            .send_simple("Target.getBrowserContexts")
            .await?;

        let context_ids = result["browserContextIds"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(context_ids)
    }

    /// Get all open targets (pages, service workers, etc.).
    pub async fn targets(&self) -> Result<Vec<TargetInfo>> {
        let result = self.session.send_simple("Target.getTargets").await?;

        let targets: Vec<TargetInfo> = result["targetInfos"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(targets)
    }

    /// Get the browser version string (e.g., "Chrome/120.0.6099.109").
    pub fn version(&self) -> &str {
        &self.version.product
    }

    /// Get the full browser version info.
    pub fn version_info(&self) -> &BrowserVersion {
        &self.version
    }

    /// Check if the browser is still connected.
    pub fn is_connected(&self) -> bool {
        self.session.is_connected()
    }

    /// Get the root CDP session for advanced operations.
    pub fn session(&self) -> &CdpSession {
        &self.session
    }

    /// Close the browser and terminate the process.
    pub async fn close(&self) -> Result<()> {
        info!("Closing browser");

        // Try graceful shutdown via CDP
        let _ = self.session.send_simple("Browser.close").await;

        // Force-kill the process if we own it
        if let Some(process_mutex) = &self.process {
            let mut process = process_mutex.lock().await;
            let _ = process.kill().await;
        }

        Ok(())
    }
}
