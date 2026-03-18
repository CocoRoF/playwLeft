//! CDP session management.
//!
//! A CdpSession wraps a Transport with an optional session ID for targeting
//! specific browser contexts or pages. It provides a higher-level API for
//! sending CDP domain commands.

use crate::error::{PlaywLeftError, Result};
use crate::protocol::transport::Transport;
use crate::protocol::types::CdpResponse;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::debug;

/// A CDP session targeting a specific browser context or page.
///
/// When the session_id is None, commands target the root browser session.
/// When set, commands are routed to the specific target (page / context).
#[derive(Clone)]
pub struct CdpSession {
    transport: Arc<Transport>,
    session_id: Option<String>,
    target_id: Option<String>,
}

impl CdpSession {
    /// Create a root session (no session ID — targets the browser itself).
    pub fn new_root(transport: Arc<Transport>) -> Self {
        Self {
            transport,
            session_id: None,
            target_id: None,
        }
    }

    /// Create a session targeting a specific page/context.
    pub fn new_target(transport: Arc<Transport>, session_id: String, target_id: String) -> Self {
        Self {
            transport,
            session_id: Some(session_id),
            target_id: Some(target_id),
        }
    }

    /// Get the session ID (if targeting a specific context/page).
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Get the target ID (if attached to a specific target).
    pub fn target_id(&self) -> Option<&str> {
        self.target_id.as_deref()
    }

    /// Send a CDP command with parameters.
    pub async fn send(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        self.transport
            .send_command(method, params, self.session_id.clone())
            .await
    }

    /// Send a CDP command with no parameters.
    pub async fn send_simple(&self, method: &str) -> Result<serde_json::Value> {
        self.send(method, serde_json::Value::Null).await
    }

    /// Send a CDP command with a timeout.
    pub async fn send_with_timeout(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout_ms: u64,
    ) -> Result<serde_json::Value> {
        self.transport
            .send_command_with_timeout(method, params, self.session_id.clone(), timeout_ms)
            .await
    }

    /// Subscribe to all CDP events on this transport.
    pub fn subscribe_events(&self) -> broadcast::Receiver<CdpResponse> {
        self.transport.subscribe_events()
    }

    /// Wait for a specific event on this session.
    pub async fn wait_for_event(&self, method: &str, timeout_ms: u64) -> Result<CdpResponse> {
        let mut rx = self.transport.subscribe_events();
        let target_method = method.to_string();
        let error_method = target_method.clone();
        let session_id = self.session_id.clone();
        let duration = std::time::Duration::from_millis(timeout_ms);

        tokio::time::timeout(duration, async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        // Match both method and session
                        if event.method.as_deref() == Some(target_method.as_str())
                            && event.session_id == session_id
                        {
                            return Ok(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PlaywLeftError::SessionClosed);
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                }
            }
        })
        .await
        .map_err(|_| PlaywLeftError::Timeout(format!("Waiting for {error_method} timed out")))?
    }

    /// Attach to a target and create a new session for it.
    pub async fn attach_to_target(&self, target_id: &str) -> Result<CdpSession> {
        debug!(target_id, "Attaching to target");

        let result = self
            .send(
                "Target.attachToTarget",
                serde_json::json!({
                    "targetId": target_id,
                    "flatten": true,
                }),
            )
            .await?;

        let session_id = result["sessionId"]
            .as_str()
            .ok_or_else(|| PlaywLeftError::ProtocolError {
                code: -1,
                message: "No sessionId in attachToTarget response".to_string(),
            })?
            .to_string();

        debug!(session_id = %session_id, target_id, "Attached to target");

        Ok(CdpSession::new_target(
            Arc::clone(&self.transport),
            session_id,
            target_id.to_string(),
        ))
    }

    /// Detach from the current target session.
    pub async fn detach(&self) -> Result<()> {
        if let Some(session_id) = &self.session_id {
            debug!(session_id = %session_id, "Detaching from target");
            // Send via root (no session_id) to detach
            self.transport
                .send_command(
                    "Target.detachFromTarget",
                    serde_json::json!({
                        "sessionId": session_id,
                    }),
                    None,
                )
                .await?;
        }
        Ok(())
    }

    /// Enable a CDP domain (e.g., "Page", "Network", "Runtime").
    pub async fn enable_domain(&self, domain: &str) -> Result<()> {
        self.send_simple(&format!("{domain}.enable")).await?;
        Ok(())
    }

    /// Disable a CDP domain.
    pub async fn disable_domain(&self, domain: &str) -> Result<()> {
        self.send_simple(&format!("{domain}.disable")).await?;
        Ok(())
    }

    /// Check if the underlying transport is still connected.
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    /// Get access to the underlying transport.
    pub fn transport(&self) -> &Arc<Transport> {
        &self.transport
    }
}
