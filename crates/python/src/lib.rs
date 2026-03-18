//! Python bindings for playwLeft.
//!
//! This crate exposes the playwLeft Rust core as a Python extension module
//! via PyO3. All async Rust methods are bridged to Python's asyncio via
//! a shared Tokio runtime.

use playleft_core::error::PlaywLeftError;
use playleft_core::{Browser, BrowserContext, BrowserType, LaunchOptions, Page};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Convert a PlaywLeftError to a Python exception.
fn to_py_err(err: PlaywLeftError) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

/// Convert a serde_json::Value to a Python object.
fn json_to_py(py: Python<'_>, val: &serde_json::Value) -> PyResult<PyObject> {
    match val {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(s) => Ok(PyString::new(py, s).into_any().unbind()),
        serde_json::Value::Array(arr) => {
            let items: Vec<PyObject> = arr
                .iter()
                .map(|v| json_to_py(py, v))
                .collect::<PyResult<_>>()?;
            Ok(PyList::new(py, &items)?.into_any().unbind())
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

// ─── PlaywLeft ───────────────────────────────────────────────────────

/// The main entry point — provides access to browser types.
#[pyclass(name = "PlaywLeft")]
struct PyPlaywLeft;

#[pymethods]
impl PyPlaywLeft {
    #[new]
    fn new() -> Self {
        Self
    }

    /// Get a Chromium browser type for launching browsers.
    fn chromium(&self) -> PyBrowserType {
        PyBrowserType {
            inner: BrowserType::chromium(),
        }
    }
}

// ─── BrowserType ─────────────────────────────────────────────────────

#[pyclass(name = "BrowserType")]
struct PyBrowserType {
    inner: BrowserType,
}

#[pymethods]
impl PyBrowserType {
    /// Launch a browser instance.
    #[pyo3(signature = (headless=true, executable_path=None, args=None, timeout=None))]
    fn launch<'py>(
        &self,
        py: Python<'py>,
        headless: bool,
        executable_path: Option<String>,
        args: Option<Vec<String>>,
        timeout: Option<u64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut opts = LaunchOptions::default();
            opts.headless = headless;
            opts.executable_path = executable_path.map(std::path::PathBuf::from);
            if let Some(a) = args {
                opts.args = a;
            }
            if let Some(t) = timeout {
                opts.timeout = t;
            }

            let browser = inner.launch(opts).await.map_err(to_py_err)?;
            Ok(PyBrowser {
                inner: Arc::new(Mutex::new(browser)),
            })
        })
    }

    /// Connect to an existing browser via CDP WebSocket URL.
    fn connect<'py>(&self, py: Python<'py>, ws_url: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let browser = inner.connect(&ws_url).await.map_err(to_py_err)?;
            Ok(PyBrowser {
                inner: Arc::new(Mutex::new(browser)),
            })
        })
    }
}

// ─── Browser ─────────────────────────────────────────────────────────

#[pyclass(name = "Browser")]
#[derive(Clone)]
struct PyBrowser {
    inner: Arc<Mutex<Browser>>,
}

#[pymethods]
impl PyBrowser {
    /// Create a new isolated browser context.
    fn new_context<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let browser = inner.lock().await;
            let ctx = browser.new_context().await.map_err(to_py_err)?;
            Ok(PyBrowserContext {
                inner: Arc::new(Mutex::new(ctx)),
            })
        })
    }

    /// Create a new page in a new context (convenience).
    fn new_page<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let browser = inner.lock().await;
            let page = browser.new_page().await.map_err(to_py_err)?;
            Ok(PyPage {
                inner: Arc::new(Mutex::new(page)),
            })
        })
    }

    /// Get the browser version string.
    fn version<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let browser = inner.lock().await;
            Ok(browser.version().to_string())
        })
    }

    /// Close the browser.
    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let browser = inner.lock().await;
            browser.close().await.map_err(to_py_err)?;
            Ok(())
        })
    }
}

// ─── BrowserContext ──────────────────────────────────────────────────

#[pyclass(name = "BrowserContext")]
#[derive(Clone)]
struct PyBrowserContext {
    inner: Arc<Mutex<BrowserContext>>,
}

#[pymethods]
impl PyBrowserContext {
    /// Create a new page in this context.
    fn new_page<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let ctx = inner.lock().await;
            let page = ctx.new_page().await.map_err(to_py_err)?;
            Ok(PyPage {
                inner: Arc::new(Mutex::new(page)),
            })
        })
    }

    /// Close this context and all its pages.
    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let ctx = inner.lock().await;
            ctx.close().await.map_err(to_py_err)?;
            Ok(())
        })
    }
}

// ─── Page ────────────────────────────────────────────────────────────

#[pyclass(name = "Page")]
#[derive(Clone)]
struct PyPage {
    inner: Arc<Mutex<Page>>,
}

#[pymethods]
impl PyPage {
    /// Navigate to a URL.
    fn goto<'py>(&self, py: Python<'py>, url: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.goto(&url).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Navigate to a URL with a custom timeout (ms).
    fn goto_with_timeout<'py>(
        &self,
        py: Python<'py>,
        url: String,
        timeout_ms: u64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.goto_with_timeout(&url, timeout_ms)
                .await
                .map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Reload the page.
    fn reload<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.reload().await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Navigate back.
    fn go_back<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.go_back().await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Navigate forward.
    fn go_forward<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.go_forward().await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Get the current page URL.
    fn url<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.url().await.map_err(to_py_err)
        })
    }

    /// Get the page title.
    fn title<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.title().await.map_err(to_py_err)
        })
    }

    /// Get the full HTML content.
    fn content<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.content().await.map_err(to_py_err)
        })
    }

    /// Set the page HTML content.
    fn set_content<'py>(&self, py: Python<'py>, html: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.set_content(&html).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Evaluate a JavaScript expression.
    fn evaluate<'py>(&self, py: Python<'py>, expression: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            let result = page.evaluate(&expression).await.map_err(to_py_err)?;
            // Convert serde_json::Value to a proper Python object
            Python::with_gil(|py| json_to_py(py, &result))
        })
    }

    /// Extract all visible text content.
    fn extract_text<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.extract_text().await.map_err(to_py_err)
        })
    }

    /// Extract all links as a list of `{ href, text }` dicts.
    fn extract_links<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            let result = page.extract_links().await.map_err(to_py_err)?;
            Python::with_gil(|py| json_to_py(py, &result))
        })
    }

    /// Extract structured data (headings, paragraphs, links, tables, forms, meta).
    fn extract_structured<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            let result = page.extract_structured().await.map_err(to_py_err)?;
            Python::with_gil(|py| json_to_py(py, &result))
        })
    }

    /// Extract the accessibility tree.
    fn extract_accessibility_tree<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            let result = page
                .extract_accessibility_tree()
                .await
                .map_err(to_py_err)?;
            Python::with_gil(|py| json_to_py(py, &result))
        })
    }

    /// Click at specific coordinates.
    fn click<'py>(&self, py: Python<'py>, x: f64, y: f64) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.click(x, y).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Type text via keyboard.
    fn type_text<'py>(&self, py: Python<'py>, text: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.type_text(&text).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Press a key (e.g., "Enter", "Tab").
    fn press_key<'py>(&self, py: Python<'py>, key: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.press_key(&key).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Wait for a selector to appear.
    #[pyo3(signature = (selector, timeout_ms=None))]
    fn wait_for_selector<'py>(
        &self,
        py: Python<'py>,
        selector: String,
        timeout_ms: Option<u64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            let _element = page
                .wait_for_selector(&selector, timeout_ms.unwrap_or(30000))
                .await
                .map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Close this page.
    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let page = inner.lock().await;
            page.close().await.map_err(to_py_err)?;
            Ok(())
        })
    }
}

// ─── Module ──────────────────────────────────────────────────────────

/// The playwLeft Python module.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyPlaywLeft>()?;
    m.add_class::<PyBrowserType>()?;
    m.add_class::<PyBrowser>()?;
    m.add_class::<PyBrowserContext>()?;
    m.add_class::<PyPage>()?;
    Ok(())
}
