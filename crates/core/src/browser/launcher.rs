//! Browser process launcher and discovery.
//!
//! Handles finding Chromium executables, spawning browser processes with
//! the correct flags, and establishing the initial CDP connection.

use crate::browser::Browser;
use crate::error::{PlaywLeftError, Result};
use crate::protocol::transport::Transport;
use crate::protocol::types::Viewport;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tracing::{debug, info};

/// Options for launching a browser instance.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Path to the browser executable. If None, auto-detect.
    pub executable_path: Option<PathBuf>,
    /// Additional command-line arguments for the browser.
    pub args: Vec<String>,
    /// Whether to run headless (always true for playwLeft, but configurable).
    pub headless: bool,
    /// User data directory. If None, uses a temporary directory.
    pub user_data_dir: Option<PathBuf>,
    /// Proxy server URL (e.g., "http://proxy:8080").
    pub proxy: Option<ProxySettings>,
    /// Default viewport for new pages.
    pub default_viewport: Option<Viewport>,
    /// Maximum time to wait for browser launch (ms).
    pub timeout: u64,
    /// Environment variables for the browser process.
    pub env: HashMap<String, String>,
    /// Port for the debugging protocol. 0 = auto-select.
    pub debugging_port: u16,
}

/// Proxy configuration.
#[derive(Debug, Clone)]
pub struct ProxySettings {
    pub server: String,
    pub bypass: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            executable_path: None,
            args: Vec::new(),
            headless: true,
            user_data_dir: None,
            proxy: None,
            default_viewport: Some(Viewport::default()),
            timeout: 30000,
            env: HashMap::new(),
            debugging_port: 0,
        }
    }
}

/// Provides methods to launch or connect to a Chromium browser.
#[derive(Debug, Clone)]
pub struct BrowserType {
    name: String,
}

impl BrowserType {
    /// Create a BrowserType for Chromium.
    pub fn chromium() -> Self {
        Self {
            name: "chromium".to_string(),
        }
    }

    /// Get the browser type name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Launch a new browser instance with default options.
    pub async fn launch_default(&self) -> Result<Browser> {
        self.launch(LaunchOptions::default()).await
    }

    /// Launch a new browser instance with the given options.
    pub async fn launch(&self, options: LaunchOptions) -> Result<Browser> {
        info!(browser = %self.name, "Launching browser");

        let executable = match &options.executable_path {
            Some(path) => path.clone(),
            None => find_chromium_executable()?,
        };

        debug!(executable = %executable.display(), "Found browser executable");

        // Create temporary user data dir if not specified
        let temp_dir = if options.user_data_dir.is_none() {
            Some(TempDir::new()?)
        } else {
            None
        };

        let user_data_dir = options
            .user_data_dir
            .clone()
            .unwrap_or_else(|| temp_dir.as_ref().unwrap().path().to_path_buf());

        // Build browser arguments
        let mut args = build_chromium_args(&options, &user_data_dir);

        // Add user-specified args
        args.extend(options.args.clone());

        debug!(args = ?args, "Browser launch arguments");

        // Spawn the browser process
        let mut cmd = Command::new(&executable);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &options.env {
            cmd.env(key, value);
        }

        let child = cmd
            .spawn()
            .map_err(|e| PlaywLeftError::BrowserError(format!("Failed to launch browser: {e}")))?;

        // Extract the debugging port from stderr
        let ws_endpoint = discover_ws_endpoint(child, options.timeout).await?;

        info!(endpoint = %ws_endpoint.ws_url, "Browser launched successfully");

        // Connect to the browser via WebSocket
        let transport = Arc::new(Transport::connect(&ws_endpoint.ws_url).await?);

        Browser::new(
            transport,
            ws_endpoint.process,
            temp_dir,
            options.default_viewport,
        )
        .await
    }

    /// Connect to an existing browser instance via its CDP WebSocket endpoint.
    pub async fn connect(&self, ws_endpoint: &str) -> Result<Browser> {
        info!(endpoint = ws_endpoint, "Connecting to existing browser");

        let transport = Arc::new(Transport::connect(ws_endpoint).await?);
        Browser::new(transport, None, None, Some(Viewport::default())).await
    }

    /// Connect to an existing browser via its HTTP debugging endpoint.
    pub async fn connect_over_cdp(&self, endpoint_url: &str) -> Result<Browser> {
        info!(endpoint = endpoint_url, "Connecting over CDP");

        // Fetch the WebSocket URL from /json/version
        let version_url = format!("{endpoint_url}/json/version");
        let client = reqwest::Client::new();
        let response: serde_json::Value = client.get(&version_url).send().await?.json().await?;

        let ws_url = response["webSocketDebuggerUrl"].as_str().ok_or_else(|| {
            PlaywLeftError::ConnectionError(
                "No webSocketDebuggerUrl in /json/version response".to_string(),
            )
        })?;

        let transport = Arc::new(Transport::connect(ws_url).await?);
        Browser::new(transport, None, None, Some(Viewport::default())).await
    }
}

/// Result of discovering the browser's WebSocket endpoint after launch.
struct WsEndpoint {
    ws_url: String,
    process: Option<Child>,
}

/// Wait for the browser to output its DevTools listening URL.
async fn discover_ws_endpoint(mut child: Child, timeout_ms: u64) -> Result<WsEndpoint> {
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| PlaywLeftError::BrowserError("Cannot read browser stderr".to_string()))?;

    let duration = std::time::Duration::from_millis(timeout_ms);

    let ws_url = tokio::time::timeout(duration, async {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            debug!(line = %line, "Browser stderr");
            // Chromium outputs: "DevTools listening on ws://127.0.0.1:PORT/devtools/browser/UUID"
            if let Some(url_start) = line.find("ws://") {
                return Ok(line[url_start..].trim().to_string());
            }
            if line.contains("Cannot start the browser process") {
                return Err(PlaywLeftError::BrowserError(line));
            }
        }
        Err(PlaywLeftError::BrowserError(
            "Browser process exited without providing WebSocket URL".to_string(),
        ))
    })
    .await
    .map_err(|_| PlaywLeftError::Timeout("Browser launch timed out".to_string()))??;

    Ok(WsEndpoint {
        ws_url,
        process: Some(child),
    })
}

/// Build Chromium command-line arguments optimized for agent use.
fn build_chromium_args(options: &LaunchOptions, user_data_dir: &std::path::Path) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", user_data_dir.display()),
        format!("--remote-debugging-port={}", options.debugging_port),
        // Disable GPU (not needed for agent use)
        "--disable-gpu".to_string(),
        // Disable unnecessary features for agent operation
        "--disable-extensions".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-sync".to_string(),
        "--disable-translate".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-background-timer-throttling".to_string(),
        "--disable-backgrounding-occluded-windows".to_string(),
        "--disable-breakpad".to_string(),
        "--disable-component-update".to_string(),
        "--disable-domain-reliability".to_string(),
        "--disable-features=TranslateUI".to_string(),
        "--disable-hang-monitor".to_string(),
        "--disable-ipc-flooding-protection".to_string(),
        "--disable-popup-blocking".to_string(),
        "--disable-prompt-on-repost".to_string(),
        "--disable-renderer-backgrounding".to_string(),
        "--disable-client-side-phishing-detection".to_string(),
        // Enable automation
        "--enable-features=NetworkService,NetworkServiceInProcess".to_string(),
        "--force-color-profile=srgb".to_string(),
        "--metrics-recording-only".to_string(),
        "--no-first-run".to_string(),
        "--password-store=basic".to_string(),
        "--use-mock-keychain".to_string(),
        // Stability
        "--no-sandbox".to_string(),
        "--disable-setuid-sandbox".to_string(),
        "--disable-dev-shm-usage".to_string(),
        // Start with about:blank
        "about:blank".to_string(),
    ];

    // Add proxy if configured
    if let Some(proxy) = &options.proxy {
        args.push(format!("--proxy-server={}", proxy.server));
        if let Some(bypass) = &proxy.bypass {
            args.push(format!("--proxy-bypass-list={bypass}"));
        }
    }

    // Add headless mode if requested
    if options.headless {
        args.push("--headless=new".to_string());
    }

    args
}

/// Find a Chromium executable on the system.
fn find_chromium_executable() -> Result<PathBuf> {
    // Try well-known executable names
    let candidates = if cfg!(target_os = "windows") {
        vec!["chrome.exe", "chromium.exe", "msedge.exe"]
    } else if cfg!(target_os = "macos") {
        vec![
            "google-chrome",
            "chromium",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ]
    } else {
        vec![
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ]
    };

    // Search in PATH
    for candidate in &candidates {
        if let Ok(path) = which::which(candidate) {
            return Ok(path);
        }
    }

    // On Windows, also check Program Files
    #[cfg(target_os = "windows")]
    {
        let program_files = [
            std::env::var("ProgramFiles").unwrap_or_default(),
            std::env::var("ProgramFiles(x86)").unwrap_or_default(),
            std::env::var("LocalAppData").unwrap_or_default(),
        ];

        let chrome_paths = [
            "Google\\Chrome\\Application\\chrome.exe",
            "Microsoft\\Edge\\Application\\msedge.exe",
            "Chromium\\Application\\chrome.exe",
        ];

        for base in &program_files {
            if base.is_empty() {
                continue;
            }
            for chrome_path in &chrome_paths {
                let full_path = PathBuf::from(base).join(chrome_path);
                if full_path.exists() {
                    return Ok(full_path);
                }
            }
        }
    }

    Err(PlaywLeftError::BrowserError(
        "Could not find a Chromium-based browser. Install Chrome, Chromium, or Edge, \
         or set executable_path in LaunchOptions."
            .to_string(),
    ))
}
