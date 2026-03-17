"""Agent workflow example — structured multi-step browser interaction."""

import asyncio
import json
from playleft import PlaywLeft


async def search_and_extract(query: str) -> dict:
    """
    Agent-style workflow: search Google and extract results.

    Returns structured search results as a dictionary.
    """
    pw = PlaywLeft()
    browser = await pw.chromium().launch()

    try:
        page = await browser.new_page()

        # Navigate to search
        search_url = f"https://www.google.com/search?q={query}"
        await page.goto(search_url)

        # Extract structured content
        structured = await page.extract_structured()
        data = json.loads(structured)

        # Extract specific search results via JS
        results_json = await page.evaluate("""
            JSON.stringify(
                Array.from(document.querySelectorAll('.g')).slice(0, 5).map(el => ({
                    title: el.querySelector('h3')?.textContent || '',
                    url: el.querySelector('a')?.href || '',
                    snippet: el.querySelector('.VwiC3b')?.textContent || ''
                }))
            )
        """)

        results = json.loads(results_json)

        return {
            "query": query,
            "page_title": data.get("title", ""),
            "results": results,
            "total_links": len(data.get("links", [])),
        }

    finally:
        await browser.close()


async def main():
    result = await search_and_extract("playwLeft browser automation rust")
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    asyncio.run(main())
