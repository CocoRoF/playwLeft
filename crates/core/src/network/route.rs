//! Route implementation for network request interception.
//!
//! Routes allow agents to intercept, modify, or block network requests.

use crate::error::Result;
use crate::protocol::session::CdpSession;
use std::collections::HashMap;

/// A route for intercepting network requests.
///
/// Routes are registered via `Page::route()` and allow modification or
/// blocking of matching requests.
pub struct Route {
    session: CdpSession,
    request_id: String,
    url: String,
    method: String,
    headers: HashMap<String, String>,
}

impl Route {
    /// Create a new Route from an intercepted request.
    pub(crate) fn new(
        session: CdpSession,
        request_id: String,
        url: String,
        method: String,
        headers: HashMap<String, String>,
    ) -> Self {
        Self {
            session,
            request_id,
            url,
            method,
            headers,
        }
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

    /// Continue the request without modification.
    pub async fn continue_request(&self) -> Result<()> {
        self.session
            .send(
                "Fetch.continueRequest",
                serde_json::json!({
                    "requestId": self.request_id,
                }),
            )
            .await?;
        Ok(())
    }

    /// Continue the request with modifications.
    pub async fn continue_with(
        &self,
        url: Option<&str>,
        method: Option<&str>,
        headers: Option<&HashMap<String, String>>,
        post_data: Option<&str>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "requestId": self.request_id,
        });

        if let Some(url) = url {
            params["url"] = serde_json::json!(url);
        }
        if let Some(method) = method {
            params["method"] = serde_json::json!(method);
        }
        if let Some(headers) = headers {
            let header_entries: Vec<serde_json::Value> = headers
                .iter()
                .map(|(k, v)| {
                    serde_json::json!({ "name": k, "value": v })
                })
                .collect();
            params["headers"] = serde_json::json!(header_entries);
        }
        if let Some(post_data) = post_data {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(post_data.as_bytes());
            params["postData"] = serde_json::json!(encoded);
        }

        self.session.send("Fetch.continueRequest", params).await?;
        Ok(())
    }

    /// Fulfill the request with a custom response (mock it).
    pub async fn fulfill(
        &self,
        status: u16,
        headers: Option<&HashMap<String, String>>,
        body: &str,
    ) -> Result<()> {
        use base64::Engine;
        let encoded_body = base64::engine::general_purpose::STANDARD.encode(body.as_bytes());

        let mut params = serde_json::json!({
            "requestId": self.request_id,
            "responseCode": status,
            "body": encoded_body,
        });

        if let Some(headers) = headers {
            let header_entries: Vec<serde_json::Value> = headers
                .iter()
                .map(|(k, v)| {
                    serde_json::json!({ "name": k, "value": v })
                })
                .collect();
            params["responseHeaders"] = serde_json::json!(header_entries);
        }

        self.session.send("Fetch.fulfillRequest", params).await?;
        Ok(())
    }

    /// Abort the request (block it).
    pub async fn abort(&self, reason: Option<&str>) -> Result<()> {
        self.session
            .send(
                "Fetch.failRequest",
                serde_json::json!({
                    "requestId": self.request_id,
                    "errorReason": reason.unwrap_or("BlockedByClient"),
                }),
            )
            .await?;
        Ok(())
    }
}
