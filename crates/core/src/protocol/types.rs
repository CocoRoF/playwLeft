//! CDP message types and domain definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A CDP request message sent to the browser.
#[derive(Debug, Clone, Serialize)]
pub struct CdpCommand {
    pub id: u64,
    pub method: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// A CDP response message received from the browser.
#[derive(Debug, Clone, Deserialize)]
pub struct CdpResponse {
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<CdpError>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub method: Option<String>,
    pub params: Option<serde_json::Value>,
}

impl CdpResponse {
    /// Returns true if this is an event (no id field).
    pub fn is_event(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }

    /// Returns true if this is a command response.
    pub fn is_response(&self) -> bool {
        self.id.is_some()
    }
}

/// CDP error structure returned on failed commands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CdpError {
    pub code: i64,
    pub message: String,
    pub data: Option<String>,
}

/// Browser version info from Browser.getVersion.
#[derive(Debug, Clone, Deserialize)]
pub struct BrowserVersion {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub product: String,
    pub revision: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
    #[serde(rename = "jsVersion")]
    pub js_version: String,
}

/// Target info from Target.getTargets / events.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetInfo {
    #[serde(rename = "targetId")]
    pub target_id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub attached: Option<bool>,
    #[serde(rename = "browserContextId")]
    pub browser_context_id: Option<String>,
    #[serde(rename = "openerId")]
    pub opener_id: Option<String>,
}

/// Frame info for page frames.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FrameInfo {
    pub id: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(rename = "loaderId")]
    pub loader_id: Option<String>,
    pub name: Option<String>,
    pub url: String,
    #[serde(rename = "securityOrigin")]
    pub security_origin: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// Remote object from JavaScript evaluation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub object_type: String,
    pub subtype: Option<String>,
    #[serde(rename = "className")]
    pub class_name: Option<String>,
    pub value: Option<serde_json::Value>,
    pub description: Option<String>,
    #[serde(rename = "objectId")]
    pub object_id: Option<String>,
    #[serde(rename = "unserializableValue")]
    pub unserializable_value: Option<String>,
}

impl RemoteObject {
    /// Extract the value from this remote object as a JSON value.
    pub fn into_value(self) -> serde_json::Value {
        if let Some(val) = self.value {
            return val;
        }
        if let Some(unserializable) = self.unserializable_value {
            match unserializable.as_str() {
                "NaN" => return serde_json::Value::Null,
                "Infinity" => return serde_json::json!(f64::INFINITY),
                "-Infinity" => return serde_json::json!(f64::NEG_INFINITY),
                "-0" => return serde_json::json!(0),
                _ => return serde_json::Value::String(unserializable),
            }
        }
        serde_json::Value::Null
    }
}

/// Exception details from Runtime.evaluate errors.
#[derive(Debug, Clone, Deserialize)]
pub struct ExceptionDetails {
    #[serde(rename = "exceptionId")]
    pub exception_id: i64,
    pub text: String,
    #[serde(rename = "lineNumber")]
    pub line_number: i64,
    #[serde(rename = "columnNumber")]
    pub column_number: i64,
    pub exception: Option<RemoteObject>,
}

/// Cookie structure for reading/writing cookies.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub size: Option<i64>,
    #[serde(rename = "httpOnly")]
    pub http_only: bool,
    pub secure: bool,
    #[serde(rename = "sameSite")]
    pub same_site: Option<String>,
    pub priority: Option<String>,
}

/// Network request data from Network domain events.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkRequest {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "loaderId")]
    pub loader_id: Option<String>,
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    #[serde(rename = "postData")]
    pub post_data: Option<String>,
    #[serde(rename = "hasPostData")]
    pub has_post_data: Option<bool>,
    #[serde(rename = "initialPriority")]
    pub initial_priority: Option<String>,
    #[serde(rename = "referrerPolicy")]
    pub referrer_policy: Option<String>,
}

/// Network response data from Network domain events.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkResponse {
    pub url: String,
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub headers: HashMap<String, String>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "remoteIPAddress")]
    pub remote_ip_address: Option<String>,
    #[serde(rename = "remotePort")]
    pub remote_port: Option<u16>,
    #[serde(rename = "securityState")]
    pub security_state: Option<String>,
}

/// Navigation entry from Page.getNavigationHistory.
#[derive(Debug, Clone, Deserialize)]
pub struct NavigationEntry {
    pub id: i64,
    pub url: String,
    #[serde(rename = "userTypedURL")]
    pub user_typed_url: String,
    pub title: String,
    #[serde(rename = "transitionType")]
    pub transition_type: String,
}

/// Viewport dimensions for emulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    #[serde(rename = "deviceScaleFactor")]
    pub device_scale_factor: f64,
    pub mobile: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            device_scale_factor: 1.0,
            mobile: false,
        }
    }
}
