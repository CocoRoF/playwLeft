//! Frame implementation — represents a frame within a page.

use crate::element::Element;
use crate::error::{PlaywLeftError, Result};
use crate::protocol::session::CdpSession;
use crate::protocol::types::RemoteObject;

/// Represents a frame within a page (main frame or iframe).
///
/// Frames provide the execution context for JavaScript and DOM operations.
pub struct Frame {
    session: CdpSession,
    frame_id: String,
    url: String,
    name: Option<String>,
    parent_frame_id: Option<String>,
}

impl Frame {
    /// Create a new Frame.
    pub(crate) fn new(
        session: CdpSession,
        frame_id: String,
        url: String,
        name: Option<String>,
        parent_frame_id: Option<String>,
    ) -> Self {
        Self {
            session,
            frame_id,
            url,
            name,
            parent_frame_id,
        }
    }

    /// Get the frame ID.
    pub fn id(&self) -> &str {
        &self.frame_id
    }

    /// Get the frame URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the frame name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Whether this is the main (root) frame.
    pub fn is_main_frame(&self) -> bool {
        self.parent_frame_id.is_none()
    }

    /// Evaluate JavaScript expression and return the result.
    pub async fn evaluate(&self, expression: &str) -> Result<serde_json::Value> {
        let result = self
            .session
            .send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;

        // Check for exceptions
        if let Some(exception) = result.get("exceptionDetails") {
            let text = exception["text"].as_str().unwrap_or("Unknown error");
            let exception_obj = exception
                .get("exception")
                .and_then(|e| e["description"].as_str())
                .unwrap_or(text);
            return Err(PlaywLeftError::EvaluationError(exception_obj.to_string()));
        }

        let remote_object: RemoteObject = serde_json::from_value(
            result
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )?;

        Ok(remote_object.into_value())
    }

    /// Evaluate JavaScript function with arguments.
    pub async fn evaluate_function(
        &self,
        function: &str,
        args: &[serde_json::Value],
    ) -> Result<serde_json::Value> {
        let call_args: Vec<serde_json::Value> = args
            .iter()
            .map(|arg| serde_json::json!({ "value": arg }))
            .collect();

        let result = self
            .session
            .send(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "functionDeclaration": function,
                    "arguments": call_args,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "executionContextId": 1,
                }),
            )
            .await?;

        if let Some(exception) = result.get("exceptionDetails") {
            let text = exception["text"].as_str().unwrap_or("Unknown error");
            return Err(PlaywLeftError::EvaluationError(text.to_string()));
        }

        let remote_object: RemoteObject = serde_json::from_value(
            result
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )?;

        Ok(remote_object.into_value())
    }

    /// Get the full HTML content of this frame.
    pub async fn content(&self) -> Result<String> {
        let result = self.evaluate("document.documentElement.outerHTML").await?;

        result.as_str().map(String::from).ok_or_else(|| {
            PlaywLeftError::EvaluationError("Failed to get page content".to_string())
        })
    }

    /// Get the page title from this frame.
    pub async fn title(&self) -> Result<String> {
        let result = self.evaluate("document.title").await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Query a single element by CSS selector.
    pub async fn query_selector(&self, selector: &str) -> Result<Option<Element>> {
        let result = self
            .session
            .send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": format!(
                        "document.querySelector({})",
                        serde_json::to_string(selector)?
                    ),
                    "returnByValue": false,
                }),
            )
            .await?;

        let remote_object: RemoteObject = serde_json::from_value(
            result
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )?;

        if remote_object.subtype.as_deref() == Some("null") || remote_object.object_id.is_none() {
            return Ok(None);
        }

        let object_id = remote_object.object_id.unwrap();
        Ok(Some(Element::new(
            self.session.clone(),
            object_id,
            selector.to_string(),
        )))
    }

    /// Query all elements matching a CSS selector.
    pub async fn query_selector_all(&self, selector: &str) -> Result<Vec<Element>> {
        let result = self
            .session
            .send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": format!(
                        "Array.from(document.querySelectorAll({}))",
                        serde_json::to_string(selector)?
                    ),
                    "returnByValue": false,
                }),
            )
            .await?;

        let remote_object: RemoteObject = serde_json::from_value(
            result
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )?;

        let Some(array_id) = remote_object.object_id else {
            return Ok(Vec::new());
        };

        // Get the array properties to extract individual elements
        let props_result = self
            .session
            .send(
                "Runtime.getProperties",
                serde_json::json!({
                    "objectId": array_id,
                    "ownProperties": true,
                }),
            )
            .await?;

        let mut elements = Vec::new();

        if let Some(properties) = props_result["result"].as_array() {
            for prop in properties {
                let name = prop["name"].as_str().unwrap_or("");
                // Only numeric indices are array elements
                if name.parse::<usize>().is_ok() {
                    if let Some(object_id) = prop["value"]["objectId"].as_str() {
                        elements.push(Element::new(
                            self.session.clone(),
                            object_id.to_string(),
                            format!("{selector}[{name}]"),
                        ));
                    }
                }
            }
        }

        Ok(elements)
    }

    /// Wait for a selector to appear in the frame.
    pub async fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<Element> {
        let poll_interval = std::time::Duration::from_millis(100);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);

        loop {
            if let Some(element) = self.query_selector(selector).await? {
                return Ok(element);
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(PlaywLeftError::Timeout(format!(
                    "Selector '{selector}' not found within {timeout_ms}ms"
                )));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Get the CDP session for this frame.
    #[allow(dead_code)]
    pub(crate) fn session(&self) -> &CdpSession {
        &self.session
    }
}
