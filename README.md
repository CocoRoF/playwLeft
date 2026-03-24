# playwLeft

**Agent-first browser automation toolkit built in Rust.**

playwLeft is a high-performance browser automation library designed primarily for AI agents
and programmatic use cases. Built in Rust with Python bindings, it provides a powerful
interface for browser testing, web scraping, and data extraction — without the overhead
of visual tooling.

## Key Features

- **Agent-First Design** — Optimized for machine-readable output, structured data extraction,
  and headless operation. No screenshots, video recording, or GUI inspectors.
- **Rust Core** — High performance, low memory footprint, strong type safety.
- **Python Library** — Distributed as a Python package with both sync and async APIs.
- **CDP Protocol** — Direct Chrome DevTools Protocol communication for precise browser control.
- **Structured Extraction** — Built-in methods to extract text, links, tables, and structured
  data from pages.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                 Python API Layer                 │
│            (sync_api / async_api)                │
├─────────────────────────────────────────────────┤
│              PyO3 Bindings Layer                 │
├─────────────────────────────────────────────────┤
│                 Rust Core                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ Browser  │ │  Page    │ │    Network       │ │
│  │ Manager  │ │  & Frame │ │  Interception    │ │
│  └────┬─────┘ └────┬─────┘ └───────┬──────────┘ │
│       │             │               │            │
│  ┌────┴─────────────┴───────────────┴──────────┐ │
│  │           CDP Protocol Layer                │ │
│  │    (WebSocket Transport + Sessions)         │ │
│  └─────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────┤
│              Chromium Browser Process             │
└─────────────────────────────────────────────────┘
```

## Quick Start (Python)

```python
import asyncio
from playwleft import PlaywLeft

async def main():
    async with PlaywLeft() as pw:
        browser = await pw.chromium.launch()
        page = await browser.new_page()

        await page.goto("https://example.com")

        # Get page content
        title = await page.title()
        content = await page.content()

        # Agent-optimized: extract structured data
        text = await page.extract_text()
        links = await page.extract_links()

        # Network interception
        await page.route("**/*.css", lambda route: route.abort())

        # JavaScript evaluation
        result = await page.evaluate("document.title")

        await browser.close()

asyncio.run(main())
```

## Installation

```bash
pip install playwleft
```

## Class Hierarchy

| Class | Description |
|-------|-------------|
| `PlaywLeft` | Entry point — provides browser type access |
| `BrowserType` | Launches or connects to browser instances |
| `Browser` | A browser instance with context management |
| `BrowserContext` | Isolated browser session (like incognito) |
| `Page` | A single browser tab with full interaction API |
| `Frame` | An iframe within a page |
| `Element` | A DOM element with query and action methods |
| `Request` | Outgoing network request |
| `Response` | Incoming network response |
| `Route` | Network request interception handler |

## Design Principles

1. **Responses over Rendering** — Every API returns structured, machine-parseable data.
2. **Headless by Default** — No headed mode; all interactions are programmatic.
3. **Minimal Surface** — Only the APIs agents actually need, no legacy compat.
4. **Performance First** — Rust core ensures minimal latency and memory usage.
5. **Deterministic** — Consistent behavior across runs, no flaky waits.

## License

Apache-2.0
