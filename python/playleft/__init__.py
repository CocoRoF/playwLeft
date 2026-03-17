"""
playwLeft — Agent-first browser automation toolkit.

Built in Rust for performance, distributed as a Python package.
Provides a Playwright-like API optimized for AI agents and programmatic use.

Usage:
    import asyncio
    from playleft import PlaywLeft

    async def main():
        pw = PlaywLeft()
        browser = await pw.chromium.launch()
        page = await browser.new_page()
        await page.goto("https://example.com")
        text = await page.extract_text()
        print(text)
        await browser.close()

    asyncio.run(main())
"""

from playleft._core import (
    PlaywLeft,
    BrowserType,
    Browser,
    BrowserContext,
    Page,
    __version__,
)

__all__ = [
    "PlaywLeft",
    "BrowserType",
    "Browser",
    "BrowserContext",
    "Page",
    "__version__",
]
