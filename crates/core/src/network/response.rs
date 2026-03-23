//! Network response wrapper.

use std::collections::HashMap;

use crate::error::Result;
use crate::protocol::session::CdpSession;

/// A captured network response.
///
/// Constructed from CDP Network domain events (Network.responseReceived).
#[derive(Debug, Clone)]
pub struct Response {
    /// The request ID this response corresponds to.
    request_id: String,
    /// The response URL (may differ from request URL due to redirects).
    url: String,
    /// HTTP status code.
    status: u16,
    /// HTTP status text.
    status_text: String,
    /// Response headers.
    headers: HashMap<String, String>,
    /// MIME type.
    mime_type: String,
    /// Remote IP address of the server.
    remote_address: Option<String>,
}

#[allow(dead_code)]
impl Response {
    /// Create a new Response from CDP event data.
    pub(crate) fn from_cdp_event(params: &serde_json::Value) -> Option<Self> {
        let response = params.get("response")?;

        let headers: HashMap<String, String> = response
            .get("headers")
            .and_then(|h| serde_json::from_value(h.clone()).ok())
            .unwrap_or_default();

        Some(Self {
            request_id: params["requestId"].as_str()?.to_string(),
            url: response["url"].as_str()?.to_string(),
            status: response["status"].as_u64().unwrap_or(0) as u16,
            status_text: response["statusText"].as_str().unwrap_or("").to_string(),
            headers,
            mime_type: response["mimeType"]
                .as_str()
                .unwrap_or("application/octet-stream")
                .to_string(),
            remote_address: response["remoteIPAddress"].as_str().map(String::from),
        })
    }

    /// Get the request ID.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Get the response URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the HTTP status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Get the HTTP status text.
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Get the response headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Get a specific header value (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }

    /// Get the MIME type.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Whether this is a successful response (2xx status).
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Get the remote server address.
    pub fn remote_address(&self) -> Option<&str> {
        self.remote_address.as_deref()
    }

    /// Fetch the response body text from the browser.
    ///
    /// Requires a CDP session to fetch the body via Network.getResponseBody.
    pub async fn body(&self, session: &CdpSession) -> Result<String> {
        let result = session
            .send(
                "Network.getResponseBody",
                serde_json::json!({
                    "requestId": self.request_id,
                }),
            )
            .await?;

        let body = result["body"].as_str().unwrap_or("").to_string();
        let base64_encoded = result["base64Encoded"].as_bool().unwrap_or(false);

        if base64_encoded {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&body)
                .map_err(|e| {
                    crate::error::PlaywLeftError::Internal(format!(
                        "Failed to decode base64 body: {e}"
                    ))
                })?;
            String::from_utf8(bytes).map_err(|e| {
                crate::error::PlaywLeftError::Internal(format!(
                    "Response body is not valid UTF-8: {e}"
                ))
            })
        } else {
            Ok(body)
        }
    }

    /// Fetch the response body as raw bytes.
    pub async fn body_bytes(&self, session: &CdpSession) -> Result<Vec<u8>> {
        let result = session
            .send(
                "Network.getResponseBody",
                serde_json::json!({
                    "requestId": self.request_id,
                }),
            )
            .await?;

        let body = result["body"].as_str().unwrap_or("");
        let base64_encoded = result["base64Encoded"].as_bool().unwrap_or(false);

        if base64_encoded {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(body)
                .map_err(|e| {
                    crate::error::PlaywLeftError::Internal(format!(
                        "Failed to decode base64 body: {e}"
                    ))
                })
        } else {
            Ok(body.as_bytes().to_vec())
        }
    }

    /// Try to parse the response body as JSON.
    pub async fn json(&self, session: &CdpSession) -> Result<serde_json::Value> {
        let text = self.body(session).await?;
        serde_json::from_str(&text).map_err(Into::into)
    }
}
