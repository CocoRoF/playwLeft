//! Error types for playwLeft.

use thiserror::Error;

/// Unified error type for all playwLeft operations.
#[derive(Error, Debug)]
pub enum PlaywLeftError {
    /// Browser process failed to launch or crashed.
    #[error("Browser error: {0}")]
    BrowserError(String),

    /// Failed to connect to browser via WebSocket.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// CDP protocol-level error returned by the browser.
    #[error("Protocol error: {message} (code: {code})")]
    ProtocolError { code: i64, message: String },

    /// Operation timed out.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Navigation failed.
    #[error("Navigation error: {0}")]
    NavigationError(String),

    /// Element not found by selector.
    #[error("Element not found: {0}")]
    ElementNotFound(String),

    /// JavaScript evaluation error.
    #[error("Evaluation error: {0}")]
    EvaluationError(String),

    /// Network interception or request error.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// The browser context or page has been closed.
    #[error("Target closed: {0}")]
    TargetClosed(String),

    /// Selector syntax error.
    #[error("Invalid selector: {0}")]
    InvalidSelector(String),

    /// Session was disconnected.
    #[error("Session closed")]
    SessionClosed,

    /// WebSocket transport error.
    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    /// Serialization / deserialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// IO error from filesystem or process operations.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic catch-all for unexpected errors.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for PlaywLeftError {
    fn from(err: serde_json::Error) -> Self {
        PlaywLeftError::SerializationError(err.to_string())
    }
}

impl From<url::ParseError> for PlaywLeftError {
    fn from(err: url::ParseError) -> Self {
        PlaywLeftError::NavigationError(format!("Invalid URL: {err}"))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for PlaywLeftError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        PlaywLeftError::WebSocketError(err.to_string())
    }
}

impl From<reqwest::Error> for PlaywLeftError {
    fn from(err: reqwest::Error) -> Self {
        PlaywLeftError::ConnectionError(err.to_string())
    }
}

/// Convenience Result type for playwLeft operations.
pub type Result<T> = std::result::Result<T, PlaywLeftError>;
