"""Type stubs for the playwLeft native extension module."""

from typing import Optional

__version__: str

class PlaywLeft:
    """Main entry point for playwLeft. Provides access to browser types."""

    def __init__(self) -> None: ...
    def chromium(self) -> BrowserType:
        """Get a Chromium browser type for launching browsers."""
        ...

class BrowserType:
    """Manages a specific browser type (Chromium)."""

    async def launch(
        self,
        headless: bool = True,
        executable_path: Optional[str] = None,
        args: Optional[list[str]] = None,
        timeout: Optional[int] = None,
    ) -> Browser:
        """Launch a new browser instance."""
        ...

    async def connect(self, ws_url: str) -> Browser:
        """Connect to an existing browser via CDP WebSocket URL."""
        ...

class Browser:
    """A running browser instance."""

    async def new_context(self) -> BrowserContext:
        """Create a new isolated browser context."""
        ...

    async def new_page(self) -> Page:
        """Create a new page in a fresh context (convenience)."""
        ...

    async def version(self) -> str:
        """Get the browser version string."""
        ...

    async def close(self) -> None:
        """Close the browser and terminate the process."""
        ...

class BrowserContext:
    """An isolated browser context (like incognito mode)."""

    async def new_page(self) -> Page:
        """Create a new page in this context."""
        ...

    async def close(self) -> None:
        """Close this context and all its pages."""
        ...

class Page:
    """A single browser page (tab)."""

    # Navigation
    async def goto(self, url: str) -> None:
        """Navigate to a URL and wait for load."""
        ...

    async def goto_with_timeout(self, url: str, timeout_ms: int) -> None:
        """Navigate to a URL with a custom timeout."""
        ...

    async def reload(self) -> None:
        """Reload the current page."""
        ...

    async def go_back(self) -> None:
        """Navigate back in history."""
        ...

    async def go_forward(self) -> None:
        """Navigate forward in history."""
        ...

    # Content
    async def url(self) -> str:
        """Get the current URL."""
        ...

    async def title(self) -> str:
        """Get the page title."""
        ...

    async def content(self) -> str:
        """Get the full HTML content."""
        ...

    async def set_content(self, html: str) -> None:
        """Set the page HTML content."""
        ...

    # JavaScript
    async def evaluate(self, expression: str) -> str:
        """Evaluate a JavaScript expression. Returns JSON string."""
        ...

    # Agent-optimized extraction
    async def extract_text(self) -> str:
        """Extract all visible text content from the page."""
        ...

    async def extract_links(self) -> str:
        """Extract all links as JSON string [{ href, text }]."""
        ...

    async def extract_structured(self) -> str:
        """Extract structured data (headings, paragraphs, links, tables, etc.) as JSON."""
        ...

    async def extract_accessibility_tree(self) -> str:
        """Extract the accessibility tree as JSON."""
        ...

    # Input
    async def click(self, x: float, y: float) -> None:
        """Click at specific coordinates."""
        ...

    async def type_text(self, text: str) -> None:
        """Type text via keyboard."""
        ...

    async def press_key(self, key: str) -> None:
        """Press a key (e.g., 'Enter', 'Tab')."""
        ...

    # Waiting
    async def wait_for_selector(
        self, selector: str, timeout_ms: Optional[int] = None
    ) -> None:
        """Wait for a CSS selector to appear on the page."""
        ...

    # Lifecycle
    async def close(self) -> None:
        """Close this page."""
        ...
