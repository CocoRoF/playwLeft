//! WebSocket transport for CDP communication.
//!
//! Manages the WebSocket connection to the Chromium browser process,
//! handles message framing, and routes responses/events to waiters.

use crate::error::{PlaywLeftError, Result};
use crate::protocol::types::{CdpCommand, CdpResponse};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, trace, warn};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Pending command awaiting a response.
struct PendingCommand {
    sender: oneshot::Sender<CdpResponse>,
}

/// WebSocket transport for CDP protocol communication.
///
/// The transport maintains a persistent WebSocket connection and provides:
/// - Command sending with automatic ID assignment
/// - Response routing to the correct waiter
/// - Event broadcasting to all listeners
pub struct Transport {
    /// Sender half for outgoing WebSocket messages.
    ws_sender: Arc<Mutex<futures_util::stream::SplitSink<WsStream, Message>>>,
    /// Auto-incrementing command ID counter.
    next_id: AtomicU64,
    /// Map of pending command IDs to their response channels.
    pending: Arc<Mutex<HashMap<u64, PendingCommand>>>,
    /// Broadcast channel for CDP events.
    event_tx: broadcast::Sender<CdpResponse>,
    /// Whether the connection is still alive.
    connected: Arc<AtomicBool>,
    /// Handle to the background reader task.
    _reader_handle: tokio::task::JoinHandle<()>,
}

impl Transport {
    /// Connect to a browser WebSocket endpoint.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        debug!(url = ws_url, "Connecting to browser WebSocket");

        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await?;
        let (ws_sender, ws_reader) = ws_stream.split();

        let pending: Arc<Mutex<HashMap<u64, PendingCommand>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(1024);
        let connected = Arc::new(AtomicBool::new(true));

        let reader_handle = {
            let pending = Arc::clone(&pending);
            let event_tx = event_tx.clone();
            let connected = Arc::clone(&connected);

            tokio::spawn(async move {
                Self::reader_loop(ws_reader, pending, event_tx, connected).await;
            })
        };

        debug!("WebSocket connection established");

        Ok(Self {
            ws_sender: Arc::new(Mutex::new(ws_sender)),
            next_id: AtomicU64::new(1),
            pending,
            event_tx,
            connected,
            _reader_handle: reader_handle,
        })
    }

    /// Background task that reads WebSocket messages and routes them.
    async fn reader_loop(
        mut reader: futures_util::stream::SplitStream<WsStream>,
        pending: Arc<Mutex<HashMap<u64, PendingCommand>>>,
        event_tx: broadcast::Sender<CdpResponse>,
        connected: Arc<AtomicBool>,
    ) {
        while let Some(msg_result) = reader.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    trace!(len = text.len(), "Received CDP message");

                    match serde_json::from_str::<CdpResponse>(&text) {
                        Ok(response) => {
                            if response.is_response() {
                                // Route to pending command waiter
                                if let Some(id) = response.id {
                                    let mut pending_map = pending.lock().await;
                                    if let Some(cmd) = pending_map.remove(&id) {
                                        let _ = cmd.sender.send(response);
                                    } else {
                                        warn!(id, "Received response for unknown command ID");
                                    }
                                }
                            } else if response.is_event() {
                                // Broadcast event to all listeners
                                let _ = event_tx.send(response);
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to parse CDP message");
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!("WebSocket connection closed by server");
                    break;
                }
                Ok(_) => {
                    // Ignore binary, ping, pong frames
                }
                Err(e) => {
                    error!(error = %e, "WebSocket read error");
                    break;
                }
            }
        }

        connected.store(false, Ordering::SeqCst);
        debug!("WebSocket reader loop terminated");

        // Notify all pending commands that the connection is closed
        let mut pending_map = pending.lock().await;
        for (id, cmd) in pending_map.drain() {
            let _ = cmd.sender.send(CdpResponse {
                id: Some(id),
                result: None,
                error: Some(crate::protocol::types::CdpError {
                    code: -1,
                    message: "Connection closed".to_string(),
                    data: None,
                }),
                session_id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Send a CDP command and wait for the response.
    pub async fn send_command(
        &self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<String>,
    ) -> Result<serde_json::Value> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(PlaywLeftError::SessionClosed);
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let command = CdpCommand {
            id,
            method: method.to_string(),
            params,
            session_id,
        };

        let (tx, rx) = oneshot::channel();

        // Register the pending command before sending
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, PendingCommand { sender: tx });
        }

        // Serialize and send
        let json = serde_json::to_string(&command)?;
        trace!(id, method = %command.method, "Sending CDP command");

        {
            let mut sender = self.ws_sender.lock().await;
            sender.send(Message::Text(json.into())).await.map_err(|e| {
                PlaywLeftError::WebSocketError(format!("Failed to send message: {e}"))
            })?;
        }

        // Wait for response
        let response = rx.await.map_err(|_| PlaywLeftError::SessionClosed)?;

        if let Some(error) = response.error {
            return Err(PlaywLeftError::ProtocolError {
                code: error.code,
                message: error.message,
            });
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }

    /// Send a CDP command with no parameters.
    pub async fn send_simple(&self, method: &str) -> Result<serde_json::Value> {
        self.send_command(method, serde_json::Value::Null, None)
            .await
    }

    /// Send a CDP command targeting a specific session.
    pub async fn send_session_command(
        &self,
        method: &str,
        params: serde_json::Value,
        session_id: &str,
    ) -> Result<serde_json::Value> {
        self.send_command(method, params, Some(session_id.to_string()))
            .await
    }

    /// Subscribe to CDP events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<CdpResponse> {
        self.event_tx.subscribe()
    }

    /// Check if the transport is still connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Send a CDP command with a timeout.
    pub async fn send_command_with_timeout(
        &self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<String>,
        timeout_ms: u64,
    ) -> Result<serde_json::Value> {
        let duration = std::time::Duration::from_millis(timeout_ms);

        tokio::time::timeout(duration, self.send_command(method, params, session_id))
            .await
            .map_err(|_| {
                PlaywLeftError::Timeout(format!("{method} timed out after {timeout_ms}ms"))
            })?
    }

    /// Wait for a specific CDP event matching the given method name.
    pub async fn wait_for_event(&self, method: &str, timeout_ms: u64) -> Result<CdpResponse> {
        let mut rx = self.event_tx.subscribe();
        let target_method = method.to_string();
        let duration = std::time::Duration::from_millis(timeout_ms);

        tokio::time::timeout(duration, async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event.method.as_deref() == Some(&target_method) {
                            return Ok(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PlaywLeftError::SessionClosed);
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Event receiver lagged behind");
                    }
                }
            }
        })
        .await
        .map_err(|_| PlaywLeftError::Timeout(format!("Waiting for {method} timed out")))?
    }

    /// Close the transport connection gracefully.
    pub async fn close(&self) -> Result<()> {
        if self.connected.load(Ordering::SeqCst) {
            let mut sender = self.ws_sender.lock().await;
            let _ = sender.close().await;
            self.connected.store(false, Ordering::SeqCst);
        }
        Ok(())
    }
}
