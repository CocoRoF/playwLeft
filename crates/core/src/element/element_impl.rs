//! Element handle implementation.
//!
//! Represents a reference to a DOM element via its remote object ID.
//! Provides methods for clicking, typing, reading attributes, and
//! querying properties — all through CDP Runtime.callFunctionOn.

use crate::error::{PlaywLeftError, Result};
use crate::protocol::session::CdpSession;
use crate::protocol::types::RemoteObject;
use tracing::debug;

/// A handle to a DOM element in the browser.
///
/// Elements are identified by their CDP remote object ID and interact
/// with the browser via `Runtime.callFunctionOn`.
pub struct Element {
    session: CdpSession,
    object_id: String,
    selector: String,
}

impl Element {
    /// Create a new Element handle.
    pub(crate) fn new(session: CdpSession, object_id: String, selector: String) -> Self {
        Self {
            session,
            object_id,
            selector,
        }
    }

    /// Get the remote object ID.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Get the selector that was used to find this element.
    pub fn selector(&self) -> &str {
        &self.selector
    }

    // ─── Properties ──────────────────────────────────────────────────

    /// Get an attribute value.
    pub async fn get_attribute(&self, name: &str) -> Result<Option<String>> {
        let result = self
            .call_on(
                &format!(
                    "function() {{ return this.getAttribute({}); }}",
                    serde_json::to_string(name)?
                ),
                &[],
            )
            .await?;

        match result {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::String(s) => Ok(Some(s)),
            other => Ok(Some(other.to_string())),
        }
    }

    /// Get the inner text of this element.
    pub async fn inner_text(&self) -> Result<String> {
        let result = self
            .call_on("function() { return this.innerText; }", &[])
            .await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Get the inner HTML of this element.
    pub async fn inner_html(&self) -> Result<String> {
        let result = self
            .call_on("function() { return this.innerHTML; }", &[])
            .await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Get the outer HTML of this element.
    pub async fn outer_html(&self) -> Result<String> {
        let result = self
            .call_on("function() { return this.outerHTML; }", &[])
            .await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Get the text content (including hidden text).
    pub async fn text_content(&self) -> Result<String> {
        let result = self
            .call_on("function() { return this.textContent; }", &[])
            .await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Get the tag name of this element (e.g., "DIV", "A", "INPUT").
    pub async fn tag_name(&self) -> Result<String> {
        let result = self
            .call_on("function() { return this.tagName; }", &[])
            .await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Get the input value (for input/textarea/select elements).
    pub async fn input_value(&self) -> Result<String> {
        let result = self
            .call_on("function() { return this.value || ''; }", &[])
            .await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Check if the element is visible.
    pub async fn is_visible(&self) -> Result<bool> {
        let result = self
            .call_on(
                r#"function() {
                    const style = window.getComputedStyle(this);
                    return style.display !== 'none'
                        && style.visibility !== 'hidden'
                        && style.opacity !== '0'
                        && this.offsetWidth > 0
                        && this.offsetHeight > 0;
                }"#,
                &[],
            )
            .await?;
        Ok(result.as_bool().unwrap_or(false))
    }

    /// Check if the element is enabled (not disabled).
    pub async fn is_enabled(&self) -> Result<bool> {
        let result = self
            .call_on("function() { return !this.disabled; }", &[])
            .await?;
        Ok(result.as_bool().unwrap_or(true))
    }

    /// Check if the element is checked (for checkboxes/radio buttons).
    pub async fn is_checked(&self) -> Result<bool> {
        let result = self
            .call_on("function() { return !!this.checked; }", &[])
            .await?;
        Ok(result.as_bool().unwrap_or(false))
    }

    // ─── Actions ─────────────────────────────────────────────────────

    /// Click this element.
    pub async fn click(&self) -> Result<()> {
        debug!(selector = %self.selector, "Clicking element");

        // Scroll into view
        self.scroll_into_view().await?;

        // Get the element's clickable point
        let box_result = self.bounding_box().await?;

        if let Some(bbox) = box_result {
            let x = bbox.x + bbox.width / 2.0;
            let y = bbox.y + bbox.height / 2.0;

            // Mouse down
            self.session
                .send(
                    "Input.dispatchMouseEvent",
                    serde_json::json!({
                        "type": "mousePressed",
                        "x": x,
                        "y": y,
                        "button": "left",
                        "clickCount": 1,
                    }),
                )
                .await?;

            // Mouse up
            self.session
                .send(
                    "Input.dispatchMouseEvent",
                    serde_json::json!({
                        "type": "mouseReleased",
                        "x": x,
                        "y": y,
                        "button": "left",
                        "clickCount": 1,
                    }),
                )
                .await?;
        } else {
            // Fallback to JS click
            self.call_on("function() { this.click(); }", &[]).await?;
        }

        Ok(())
    }

    /// Double-click this element.
    pub async fn dblclick(&self) -> Result<()> {
        self.scroll_into_view().await?;

        if let Some(bbox) = self.bounding_box().await? {
            let x = bbox.x + bbox.width / 2.0;
            let y = bbox.y + bbox.height / 2.0;

            self.session
                .send(
                    "Input.dispatchMouseEvent",
                    serde_json::json!({
                        "type": "mousePressed",
                        "x": x,
                        "y": y,
                        "button": "left",
                        "clickCount": 2,
                    }),
                )
                .await?;

            self.session
                .send(
                    "Input.dispatchMouseEvent",
                    serde_json::json!({
                        "type": "mouseReleased",
                        "x": x,
                        "y": y,
                        "button": "left",
                        "clickCount": 2,
                    }),
                )
                .await?;
        } else {
            self.call_on(
                "function() { this.dispatchEvent(new MouseEvent('dblclick', {bubbles: true})); }",
                &[],
            )
            .await?;
        }

        Ok(())
    }

    /// Type text into this element (clears existing text first).
    pub async fn fill(&self, value: &str) -> Result<()> {
        debug!(selector = %self.selector, "Filling element");

        // Focus the element
        self.focus().await?;

        // Clear existing content and set new value
        self.call_on(
            r#"function(v) {
                this.value = '';
                this.value = v;
                this.dispatchEvent(new Event('input', { bubbles: true }));
                this.dispatchEvent(new Event('change', { bubbles: true }));
            }"#,
            &[serde_json::json!(value)],
        )
        .await?;

        Ok(())
    }

    /// Type text character by character (simulating real keystrokes).
    pub async fn type_text(&self, text: &str) -> Result<()> {
        self.focus().await?;

        for ch in text.chars() {
            self.session
                .send(
                    "Input.dispatchKeyEvent",
                    serde_json::json!({
                        "type": "keyDown",
                        "text": ch.to_string(),
                    }),
                )
                .await?;
            self.session
                .send(
                    "Input.dispatchKeyEvent",
                    serde_json::json!({
                        "type": "keyUp",
                        "text": ch.to_string(),
                    }),
                )
                .await?;
        }

        Ok(())
    }

    /// Press a key on this element (e.g., "Enter", "Tab").
    pub async fn press(&self, key: &str) -> Result<()> {
        self.focus().await?;

        self.session
            .send(
                "Input.dispatchKeyEvent",
                serde_json::json!({
                    "type": "keyDown",
                    "key": key,
                }),
            )
            .await?;
        self.session
            .send(
                "Input.dispatchKeyEvent",
                serde_json::json!({
                    "type": "keyUp",
                    "key": key,
                }),
            )
            .await?;

        Ok(())
    }

    /// Focus this element.
    pub async fn focus(&self) -> Result<()> {
        self.call_on("function() { this.focus(); }", &[]).await?;
        Ok(())
    }

    /// Hover over this element.
    pub async fn hover(&self) -> Result<()> {
        self.scroll_into_view().await?;

        if let Some(bbox) = self.bounding_box().await? {
            let x = bbox.x + bbox.width / 2.0;
            let y = bbox.y + bbox.height / 2.0;

            self.session
                .send(
                    "Input.dispatchMouseEvent",
                    serde_json::json!({
                        "type": "mouseMoved",
                        "x": x,
                        "y": y,
                    }),
                )
                .await?;
        }

        Ok(())
    }

    /// Select an option by value (for <select> elements).
    pub async fn select_option(&self, value: &str) -> Result<()> {
        self.call_on(
            r#"function(val) {
                const options = Array.from(this.options);
                const option = options.find(o => o.value === val);
                if (option) {
                    this.value = val;
                    option.selected = true;
                    this.dispatchEvent(new Event('input', { bubbles: true }));
                    this.dispatchEvent(new Event('change', { bubbles: true }));
                }
            }"#,
            &[serde_json::json!(value)],
        )
        .await?;

        Ok(())
    }

    /// Check a checkbox or radio button.
    pub async fn check(&self) -> Result<()> {
        let checked = self.is_checked().await?;
        if !checked {
            self.click().await?;
        }
        Ok(())
    }

    /// Uncheck a checkbox.
    pub async fn uncheck(&self) -> Result<()> {
        let checked = self.is_checked().await?;
        if checked {
            self.click().await?;
        }
        Ok(())
    }

    /// Scroll this element into view.
    pub async fn scroll_into_view(&self) -> Result<()> {
        self.call_on("function() { this.scrollIntoViewIfNeeded(true); }", &[])
            .await?;
        Ok(())
    }

    // ─── Sub-queries ─────────────────────────────────────────────────

    /// Query a child element by CSS selector.
    pub async fn query_selector(&self, selector: &str) -> Result<Option<Element>> {
        let result = self
            .session
            .send(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "functionDeclaration": format!(
                        "function() {{ return this.querySelector({}); }}",
                        serde_json::to_string(selector)?
                    ),
                    "objectId": self.object_id,
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

        let oid = remote_object.object_id.unwrap();
        Ok(Some(Element::new(
            self.session.clone(),
            oid,
            selector.to_string(),
        )))
    }

    /// Query all child elements matching a CSS selector.
    pub async fn query_selector_all(&self, selector: &str) -> Result<Vec<Element>> {
        let result = self
            .session
            .send(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "functionDeclaration": format!(
                        "function() {{ return Array.from(this.querySelectorAll({})); }}",
                        serde_json::to_string(selector)?
                    ),
                    "objectId": self.object_id,
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
                if name.parse::<usize>().is_ok() {
                    if let Some(oid) = prop["value"]["objectId"].as_str() {
                        elements.push(Element::new(
                            self.session.clone(),
                            oid.to_string(),
                            format!("{selector}[{name}]"),
                        ));
                    }
                }
            }
        }

        Ok(elements)
    }

    // ─── Internal helpers ────────────────────────────────────────────

    /// Call a JavaScript function on this element.
    async fn call_on(
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
                    "objectId": self.object_id,
                    "arguments": call_args,
                    "returnByValue": true,
                    "awaitPromise": true,
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

    /// Get the bounding box of this element.
    async fn bounding_box(&self) -> Result<Option<BoundingBox>> {
        let result = self
            .call_on(
                r#"function() {
                    const rect = this.getBoundingClientRect();
                    if (rect.width === 0 && rect.height === 0) return null;
                    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
                }"#,
                &[],
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        Ok(Some(BoundingBox {
            x: result["x"].as_f64().unwrap_or(0.0),
            y: result["y"].as_f64().unwrap_or(0.0),
            width: result["width"].as_f64().unwrap_or(0.0),
            height: result["height"].as_f64().unwrap_or(0.0),
        }))
    }
}

/// The bounding box of an element on the page.
struct BoundingBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
