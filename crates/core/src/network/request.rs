//! Network request wrapper.

use std::collections::HashMap;

/// A captured network request.
///
/// Constructed from CDP Network domain events (Network.requestWillBeSent).
#[derive(Debug, Clone)]
pub struct Request {
    /// Unique request ID assigned by the browser.
    request_id: String,
    /// The URL of the request.
    url: String,
    /// HTTP method (GET, POST, etc.).
    method: String,
    /// Request headers.
    headers: HashMap<String, String>,
    /// POST data if present.
    post_data: Option<String>,
    /// Resource type (Document, Script, Stylesheet, etc.).
    resource_type: String,
    /// Whether this request was intercepted for modification.
    is_intercepted: bool,
}

impl Request {
    /// Create a new Request from CDP event data.
    pub(crate) fn from_cdp_event(params: &serde_json::Value) -> Option<Self> {
        let request = params.get("request")?;

        let headers: HashMap<String, String> = request
            .get("headers")
            .and_then(|h| serde_json::from_value(h.clone()).ok())
            .unwrap_or_default();

        Some(Self {
            request_id: params["requestId"].as_str()?.to_string(),
            url: request["url"].as_str()?.to_string(),
            method: request["method"].as_str().unwrap_or("GET").to_string(),
            headers,
            post_data: request["postData"].as_str().map(String::from),
            resource_type: params["type"].as_str().unwrap_or("Other").to_string(),
            is_intercepted: false,
        })
    }

    /// Get the request ID.
    pub fn id(&self) -> &str {
        &self.request_id
    }

    /// Get the request URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the HTTP method.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Get the request headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Get the POST data body, if any.
    pub fn post_data(&self) -> Option<&str> {
        self.post_data.as_deref()
    }

    /// Get the resource type (Document, Script, Stylesheet, Image, etc.).
    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }

    /// Whether this request was intercepted.
    pub fn is_intercepted(&self) -> bool {
        self.is_intercepted
    }
}
