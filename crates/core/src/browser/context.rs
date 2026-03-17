//! Browser context management.
//!
//! A BrowserContext is an isolated browser session, similar to an incognito window.
//! Each context has its own cookies, cache, and storage.

use crate::error::{PlaywLeftError, Result};
use crate::page::Page;
use crate::protocol::session::CdpSession;
use crate::protocol::types::{Cookie, TargetInfo, Viewport};
use std::collections::HashMap;
use tracing::debug;

/// An isolated browser context (like incognito mode).
///
/// Each context maintains independent:
/// - Cookies and storage
/// - Cache
/// - Authentication state
/// - Proxy settings (if configured)
pub struct BrowserContext {
    /// Root CDP session for context-level commands.
    session: CdpSession,
    /// The context ID assigned by the browser.
    context_id: String,
    /// Default viewport for pages in this context.
    default_viewport: Option<Viewport>,
    /// Extra HTTP headers applied to all requests.
    extra_headers: HashMap<String, String>,
}

impl BrowserContext {
    /// Create a new BrowserContext (called internally by Browser).
    pub(crate) async fn new(
        session: CdpSession,
        context_id: String,
        default_viewport: Option<Viewport>,
    ) -> Result<Self> {
        Ok(Self {
            session,
            context_id,
            default_viewport,
            extra_headers: HashMap::new(),
        })
    }

    /// Create a new page in this context.
    pub async fn new_page(&self) -> Result<Page> {
        let result = self
            .session
            .send(
                "Target.createTarget",
                serde_json::json!({
                    "url": "about:blank",
                    "browserContextId": self.context_id,
                }),
            )
            .await?;

        let target_id = result["targetId"]
            .as_str()
            .ok_or_else(|| {
                PlaywLeftError::ProtocolError {
                    code: -1,
                    message: "No targetId in createTarget response".to_string(),
                }
            })?
            .to_string();

        debug!(target_id = %target_id, "Created new page target");

        // Attach to the target to get a session
        let page_session = self.session.attach_to_target(&target_id).await?;

        Page::new(page_session, target_id, self.default_viewport.clone()).await
    }

    /// Get the context ID.
    pub fn id(&self) -> &str {
        &self.context_id
    }

    /// Get all pages in this context.
    pub async fn pages(&self) -> Result<Vec<TargetInfo>> {
        let result = self.session.send_simple("Target.getTargets").await?;

        let pages: Vec<TargetInfo> = result["targetInfos"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value::<TargetInfo>(v.clone()).ok())
                    .filter(|t| {
                        t.target_type == "page"
                            && t.browser_context_id.as_deref() == Some(&self.context_id)
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(pages)
    }

    /// Add cookies to this context.
    pub async fn add_cookies(&self, cookies: &[Cookie]) -> Result<()> {
        self.session
            .send(
                "Storage.setCookies",
                serde_json::json!({
                    "cookies": cookies,
                    "browserContextId": self.context_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Get all cookies in this context.
    pub async fn cookies(&self, urls: Option<&[&str]>) -> Result<Vec<Cookie>> {
        let mut params = serde_json::json!({
            "browserContextId": self.context_id,
        });

        if let Some(urls) = urls {
            params["urls"] = serde_json::json!(urls);
        }

        let result = self.session.send("Storage.getCookies", params).await?;

        let cookies: Vec<Cookie> = result["cookies"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(cookies)
    }

    /// Clear all cookies in this context.
    pub async fn clear_cookies(&self) -> Result<()> {
        self.session
            .send(
                "Storage.clearCookies",
                serde_json::json!({
                    "browserContextId": self.context_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Set extra HTTP headers for all requests in this context.
    pub async fn set_extra_http_headers(&mut self, headers: HashMap<String, String>) -> Result<()> {
        self.extra_headers = headers;
        Ok(())
    }

    /// Grant permissions to this context.
    pub async fn grant_permissions(
        &self,
        permissions: &[&str],
        origin: Option<&str>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "permissions": permissions,
            "browserContextId": self.context_id,
        });

        if let Some(origin) = origin {
            params["origin"] = serde_json::json!(origin);
        }

        self.session
            .send("Browser.grantPermissions", params)
            .await?;
        Ok(())
    }

    /// Set geolocation for this context.
    pub async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: Option<f64>,
    ) -> Result<()> {
        self.session
            .send(
                "Emulation.setGeolocationOverride",
                serde_json::json!({
                    "latitude": latitude,
                    "longitude": longitude,
                    "accuracy": accuracy.unwrap_or(0.0),
                }),
            )
            .await?;
        Ok(())
    }

    /// Set offline mode for this context.
    pub async fn set_offline(&self, offline: bool) -> Result<()> {
        self.session
            .send(
                "Network.emulateNetworkConditions",
                serde_json::json!({
                    "offline": offline,
                    "latency": 0,
                    "downloadThroughput": -1,
                    "uploadThroughput": -1,
                }),
            )
            .await?;
        Ok(())
    }

    /// Close this context and all its pages.
    pub async fn close(&self) -> Result<()> {
        debug!(context_id = %self.context_id, "Closing browser context");
        self.session
            .send(
                "Target.disposeBrowserContext",
                serde_json::json!({
                    "browserContextId": self.context_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Get the root session for advanced operations.
    pub fn session(&self) -> &CdpSession {
        &self.session
    }
}
