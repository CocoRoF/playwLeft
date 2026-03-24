"""Basic navigation example for playwLeft."""

import asyncio
from playwleft import PlaywLeft


async def main():
    pw = PlaywLeft()
    browser = await pw.chromium().launch()

    try:
        page = await browser.new_page()
        await page.goto("https://example.com")

        # Get page info
        title = await page.title()
        url = await page.url()
        print(f"Title: {title}")
        print(f"URL: {url}")

        # Extract visible text
        text = await page.extract_text()
        print(f"\n--- Page Text ---\n{text}")

        # Extract all links
        links = await page.extract_links()
        print(f"\n--- Links ---\n{links}")

    finally:
        await browser.close()


if __name__ == "__main__":
    asyncio.run(main())
