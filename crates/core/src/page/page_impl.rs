//! Page implementation — navigation, content extraction, JS evaluation.
//!
//! A Page represents a single tab or popup in the browser. It is the primary
//! interface for agent interaction: navigating, evaluating JavaScript,
//! extracting structured data, and querying DOM elements.

use crate::element::Element;
use crate::error::{PlaywLeftError, Result};
use crate::page::frame_impl::Frame;
use crate::protocol::session::CdpSession;
use crate::protocol::types::Viewport;
use tracing::debug;

/// Represents a single browser page (tab).
pub struct Page {
    /// CDP session scoped to this page's target.
    session: CdpSession,
    /// Target ID for this page.
    target_id: String,
    /// The main frame of the page.
    main_frame: Frame,
}

impl Page {
    /// Create a new Page and initialize CDP domains.
    pub(crate) async fn new(
        session: CdpSession,
        target_id: String,
        viewport: Option<Viewport>,
    ) -> Result<Self> {
        // Enable required domains
        session.enable_domain("Page").await?;
        session.enable_domain("Runtime").await?;
        session.enable_domain("Network").await?;
        session.enable_domain("DOM").await?;

        // Set viewport if specified
        if let Some(vp) = &viewport {
            session
                .send(
                    "Emulation.setDeviceMetricsOverride",
                    serde_json::json!({
                        "width": vp.width,
                        "height": vp.height,
                        "deviceScaleFactor": vp.device_scale_factor,
                        "mobile": vp.mobile,
                    }),
                )
                .await?;
        }

        // Get the frame tree to find the main frame ID
        let frame_tree = session.send_simple("Page.getFrameTree").await?;
        let frame_id = frame_tree["frameTree"]["frame"]["id"]
            .as_str()
            .unwrap_or("main")
            .to_string();
        let frame_url = frame_tree["frameTree"]["frame"]["url"]
            .as_str()
            .unwrap_or("about:blank")
            .to_string();

        let main_frame = Frame::new(
            session.clone(),
            frame_id,
            frame_url,
            None,
            None, // main frame has no parent
        );

        debug!(target_id = %target_id, "Page initialized");

        Ok(Self {
            session,
            target_id,
            main_frame,
        })
    }

    // ─── Navigation ──────────────────────────────────────────────────

    /// Navigate to a URL and wait for the load event.
    pub async fn goto(&self, url: &str) -> Result<()> {
        // Validate URL
        let _parsed = url::Url::parse(url)?;

        debug!(url, "Navigating to URL");

        // Subscribe to load event BEFORE sending navigate to avoid race condition
        let mut rx = self.session.subscribe_events();
        let load_fut = {
            let session_id = self.session.session_id().map(String::from);
            async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if event.method.as_deref() == Some("Page.loadEventFired")
                                && event.session_id == session_id
                            {
                                return Ok::<(), PlaywLeftError>(());
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            return Err(PlaywLeftError::SessionClosed);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
            }
        };

        let result = self
            .session
            .send("Page.navigate", serde_json::json!({ "url": url }))
            .await?;

        // Check for navigation error
        if let Some(error_text) = result.get("errorText").and_then(|v| v.as_str()) {
            if !error_text.is_empty() {
                return Err(PlaywLeftError::NavigationError(error_text.to_string()));
            }
        }

        // Wait for the load event with timeout
        tokio::time::timeout(std::time::Duration::from_millis(30000), load_fut)
            .await
            .map_err(|_| PlaywLeftError::Timeout("Page load timed out".to_string()))??;

        Ok(())
    }

    /// Navigate to a URL with a custom timeout.
    pub async fn goto_with_timeout(&self, url: &str, timeout_ms: u64) -> Result<()> {
        let _parsed = url::Url::parse(url)?;

        // Subscribe to load event BEFORE navigating
        let mut rx = self.session.subscribe_events();
        let session_id = self.session.session_id().map(String::from);
        let load_fut = async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event.method.as_deref() == Some("Page.loadEventFired")
                            && event.session_id == session_id
                        {
                            return Ok::<(), PlaywLeftError>(());
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(PlaywLeftError::SessionClosed);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        };

        let result = self
            .session
            .send_with_timeout(
                "Page.navigate",
                serde_json::json!({ "url": url }),
                timeout_ms,
            )
            .await?;

        if let Some(error_text) = result.get("errorText").and_then(|v| v.as_str()) {
            if !error_text.is_empty() {
                return Err(PlaywLeftError::NavigationError(error_text.to_string()));
            }
        }

        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), load_fut)
            .await
            .map_err(|_| PlaywLeftError::Timeout("Page load timed out".to_string()))??;

        Ok(())
    }

    /// Reload the current page.
    pub async fn reload(&self) -> Result<()> {
        self.session.send_simple("Page.reload").await?;
        self.wait_for_load_state("load", 30000).await?;
        Ok(())
    }

    /// Navigate back in history.
    pub async fn go_back(&self) -> Result<()> {
        let nav = self
            .session
            .send_simple("Page.getNavigationHistory")
            .await?;

        let current_index = nav["currentIndex"].as_i64().unwrap_or(0);
        if current_index > 0 {
            let entries = nav["entries"].as_array();
            if let Some(entries) = entries {
                if let Some(entry) = entries.get((current_index - 1) as usize) {
                    let entry_id = entry["id"].as_i64().unwrap_or(0);
                    self.session
                        .send(
                            "Page.navigateToHistoryEntry",
                            serde_json::json!({ "entryId": entry_id }),
                        )
                        .await?;
                    self.wait_for_load_state("load", 30000).await?;
                }
            }
        }
        Ok(())
    }

    /// Navigate forward in history.
    pub async fn go_forward(&self) -> Result<()> {
        let nav = self
            .session
            .send_simple("Page.getNavigationHistory")
            .await?;

        let current_index = nav["currentIndex"].as_i64().unwrap_or(0) as usize;
        let entries = nav["entries"].as_array();
        if let Some(entries) = entries {
            if current_index + 1 < entries.len() {
                if let Some(entry) = entries.get(current_index + 1) {
                    let entry_id = entry["id"].as_i64().unwrap_or(0);
                    self.session
                        .send(
                            "Page.navigateToHistoryEntry",
                            serde_json::json!({ "entryId": entry_id }),
                        )
                        .await?;
                    self.wait_for_load_state("load", 30000).await?;
                }
            }
        }
        Ok(())
    }

    // ─── Waiting ─────────────────────────────────────────────────────

    /// Wait for a specific page lifecycle event.
    ///
    /// Common states: "load", "DOMContentLoaded", "networkIdle"
    pub async fn wait_for_load_state(&self, state: &str, timeout_ms: u64) -> Result<()> {
        match state {
            "load" => {
                self.session
                    .wait_for_event("Page.loadEventFired", timeout_ms)
                    .await?;
            }
            "DOMContentLoaded" | "domcontentloaded" => {
                self.session
                    .wait_for_event("Page.domContentEventFired", timeout_ms)
                    .await?;
            }
            "networkIdle" | "networkidle" => {
                // Wait for load first, then a brief period with no network activity
                self.session
                    .wait_for_event("Page.loadEventFired", timeout_ms)
                    .await?;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            _ => {
                return Err(PlaywLeftError::Internal(format!(
                    "Unknown load state: {state}"
                )));
            }
        }
        Ok(())
    }

    /// Wait for navigation to complete (e.g., after clicking a link).
    pub async fn wait_for_navigation(&self, timeout_ms: u64) -> Result<()> {
        self.session
            .wait_for_event("Page.frameNavigated", timeout_ms)
            .await?;
        self.wait_for_load_state("load", timeout_ms).await?;
        Ok(())
    }

    /// Wait for a CSS selector to appear on the page.
    pub async fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<Element> {
        self.main_frame
            .wait_for_selector(selector, timeout_ms)
            .await
    }

    // ─── Content ─────────────────────────────────────────────────────

    /// Get the current URL of the page.
    pub async fn url(&self) -> Result<String> {
        let result = self.evaluate("window.location.href").await?;
        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Get the page title.
    pub async fn title(&self) -> Result<String> {
        self.main_frame.title().await
    }

    /// Get the full HTML content of the page.
    pub async fn content(&self) -> Result<String> {
        self.main_frame.content().await
    }

    /// Set the page HTML content.
    pub async fn set_content(&self, html: &str) -> Result<()> {
        // Subscribe to load event BEFORE setting content
        let mut rx = self.session.subscribe_events();
        let session_id = self.session.session_id().map(String::from);
        let load_fut = async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event.method.as_deref() == Some("Page.loadEventFired")
                            && event.session_id == session_id
                        {
                            return Ok::<(), PlaywLeftError>(());
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(PlaywLeftError::SessionClosed);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        };

        self.session
            .send(
                "Page.setDocumentContent",
                serde_json::json!({
                    "frameId": self.main_frame.id(),
                    "html": html,
                }),
            )
            .await?;

        // Wait for load event so scripts in the HTML have executed
        tokio::time::timeout(std::time::Duration::from_millis(30000), load_fut)
            .await
            .map_err(|_| PlaywLeftError::Timeout("set_content load timed out".to_string()))??;

        Ok(())
    }

    // ─── JavaScript Evaluation ───────────────────────────────────────

    /// Evaluate a JavaScript expression and return the result.
    pub async fn evaluate(&self, expression: &str) -> Result<serde_json::Value> {
        self.main_frame.evaluate(expression).await
    }

    /// Evaluate a JavaScript function with arguments.
    pub async fn evaluate_function(
        &self,
        function: &str,
        args: &[serde_json::Value],
    ) -> Result<serde_json::Value> {
        self.main_frame.evaluate_function(function, args).await
    }

    /// Add a script to evaluate on every new document.
    pub async fn add_init_script(&self, script: &str) -> Result<()> {
        self.session
            .send(
                "Page.addScriptToEvaluateOnNewDocument",
                serde_json::json!({ "source": script }),
            )
            .await?;
        Ok(())
    }

    // ─── DOM Queries ─────────────────────────────────────────────────

    /// Query a single element by CSS selector.
    pub async fn query_selector(&self, selector: &str) -> Result<Option<Element>> {
        self.main_frame.query_selector(selector).await
    }

    /// Query all elements matching a CSS selector.
    pub async fn query_selector_all(&self, selector: &str) -> Result<Vec<Element>> {
        self.main_frame.query_selector_all(selector).await
    }

    // ─── Agent-Optimized Extraction ──────────────────────────────────
    //
    // These methods return clean, structured data suitable for AI/LLM
    // consumption — no raw HTML needed.

    /// Extract all visible text content from the page.
    ///
    /// Returns clean text with whitespace normalized, suitable for agent
    /// consumption without parsing HTML.
    pub async fn extract_text(&self) -> Result<String> {
        let result = self
            .evaluate(
                r#"
                (function() {
                    const walker = document.createTreeWalker(
                        document.body,
                        NodeFilter.SHOW_TEXT,
                        {
                            acceptNode: function(node) {
                                const parent = node.parentElement;
                                if (!parent) return NodeFilter.FILTER_REJECT;
                                const tag = parent.tagName.toLowerCase();
                                if (tag === 'script' || tag === 'style' || tag === 'noscript')
                                    return NodeFilter.FILTER_REJECT;
                                const style = window.getComputedStyle(parent);
                                if (style.display === 'none' || style.visibility === 'hidden')
                                    return NodeFilter.FILTER_REJECT;
                                const text = node.textContent.trim();
                                if (text.length === 0) return NodeFilter.FILTER_REJECT;
                                return NodeFilter.FILTER_ACCEPT;
                            }
                        }
                    );
                    const texts = [];
                    let node;
                    while ((node = walker.nextNode())) {
                        texts.push(node.textContent.trim());
                    }
                    return texts.join('\n');
                })()
                "#,
            )
            .await?;

        Ok(result.as_str().unwrap_or("").to_string())
    }

    /// Extract all links from the page.
    ///
    /// Returns a JSON array of `{ href, text }` objects.
    pub async fn extract_links(&self) -> Result<serde_json::Value> {
        self.evaluate(
            r#"
            Array.from(document.querySelectorAll('a[href]')).map(a => ({
                href: a.href,
                text: a.textContent.trim()
            }))
            "#,
        )
        .await
    }

    /// Extract structured data from the page as JSON.
    ///
    /// Extracts headings, paragraphs, lists, tables, and forms into a
    /// machine-readable structure. This is the primary method for agents
    /// that need to understand page content.
    pub async fn extract_structured(&self) -> Result<serde_json::Value> {
        self.evaluate(
            r#"
            (function() {
                const result = {
                    title: document.title,
                    url: window.location.href,
                    headings: [],
                    paragraphs: [],
                    links: [],
                    lists: [],
                    tables: [],
                    forms: [],
                    meta: {}
                };

                // Headings
                document.querySelectorAll('h1,h2,h3,h4,h5,h6').forEach(h => {
                    result.headings.push({
                        level: parseInt(h.tagName[1]),
                        text: h.textContent.trim()
                    });
                });

                // Paragraphs
                document.querySelectorAll('p').forEach(p => {
                    const text = p.textContent.trim();
                    if (text) result.paragraphs.push(text);
                });

                // Links
                document.querySelectorAll('a[href]').forEach(a => {
                    result.links.push({
                        href: a.href,
                        text: a.textContent.trim()
                    });
                });

                // Lists
                document.querySelectorAll('ul, ol').forEach(list => {
                    const items = Array.from(list.querySelectorAll(':scope > li'))
                        .map(li => li.textContent.trim());
                    if (items.length > 0) {
                        result.lists.push({
                            type: list.tagName.toLowerCase(),
                            items: items
                        });
                    }
                });

                // Tables
                document.querySelectorAll('table').forEach(table => {
                    const headers = Array.from(table.querySelectorAll('thead th, tr:first-child th'))
                        .map(th => th.textContent.trim());
                    const rows = [];
                    table.querySelectorAll('tbody tr, tr').forEach((tr, i) => {
                        if (i === 0 && headers.length > 0) return;
                        const cells = Array.from(tr.querySelectorAll('td, th'))
                            .map(td => td.textContent.trim());
                        if (cells.length > 0) rows.push(cells);
                    });
                    result.tables.push({ headers, rows });
                });

                // Forms
                document.querySelectorAll('form').forEach(form => {
                    const fields = Array.from(form.querySelectorAll('input, select, textarea'))
                        .map(el => ({
                            tag: el.tagName.toLowerCase(),
                            type: el.type || null,
                            name: el.name || null,
                            id: el.id || null,
                            placeholder: el.placeholder || null,
                            required: el.required || false
                        }));
                    result.forms.push({
                        action: form.action,
                        method: form.method,
                        fields: fields
                    });
                });

                // Meta tags
                document.querySelectorAll('meta[name], meta[property]').forEach(meta => {
                    const key = meta.getAttribute('name') || meta.getAttribute('property');
                    const value = meta.getAttribute('content');
                    if (key && value) result.meta[key] = value;
                });

                return result;
            })()
            "#,
        )
        .await
    }

    /// Extract the accessibility tree in a simplified format.
    ///
    /// Uses the CDP Accessibility domain to return a tree of accessible nodes.
    pub async fn extract_accessibility_tree(&self) -> Result<serde_json::Value> {
        self.session.enable_domain("Accessibility").await?;

        let result = self
            .session
            .send("Accessibility.getFullAXTree", serde_json::json!({}))
            .await?;

        Ok(result)
    }

    // ─── Input ───────────────────────────────────────────────────────

    /// Click at specific coordinates on the page.
    pub async fn click(&self, x: f64, y: f64) -> Result<()> {
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

        Ok(())
    }

    /// Type text via keyboard input events.
    pub async fn type_text(&self, text: &str) -> Result<()> {
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

    /// Press a specific key (e.g., "Enter", "Tab", "Escape").
    pub async fn press_key(&self, key: &str) -> Result<()> {
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

    // ─── Lifecycle ───────────────────────────────────────────────────

    /// Get the target ID of this page.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Get a reference to the main frame.
    pub fn main_frame(&self) -> &Frame {
        &self.main_frame
    }

    /// Get the CDP session for advanced operations.
    pub fn session(&self) -> &CdpSession {
        &self.session
    }

    /// Close this page.
    pub async fn close(&self) -> Result<()> {
        debug!(target_id = %self.target_id, "Closing page");
        self.session
            .send(
                "Target.closeTarget",
                serde_json::json!({ "targetId": self.target_id }),
            )
            .await?;
        Ok(())
    }
}
