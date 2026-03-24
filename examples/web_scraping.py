"""Web scraping example — extracting structured data from a page."""

import asyncio
import json
from playwleft import PlaywLeft


async def main():
    pw = PlaywLeft()
    browser = await pw.chromium().launch()

    try:
        page = await browser.new_page()
        await page.goto("https://news.ycombinator.com")

        # Extract structured page data (headings, links, tables, etc.)
        structured = await page.extract_structured()
        data = json.loads(structured)

        print(f"Page: {data.get('title', 'N/A')}")
        print(f"URL: {data.get('url', 'N/A')}")

        # Print headings
        print("\n--- Headings ---")
        for heading in data.get("headings", []):
            indent = "  " * (heading["level"] - 1)
            print(f"{indent}H{heading['level']}: {heading['text']}")

        # Print first 10 links
        print("\n--- Links (first 10) ---")
        for link in data.get("links", [])[:10]:
            print(f"  [{link['text'][:50]}] -> {link['href']}")

        # Evaluate custom JavaScript
        result = await page.evaluate("document.querySelectorAll('.titleline > a').length")
        print(f"\nNumber of story links: {result}")

    finally:
        await browser.close()


if __name__ == "__main__":
    asyncio.run(main())
