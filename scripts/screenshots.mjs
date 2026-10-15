/**
 * Regenerates the README screenshots in docs/screenshots/.
 *
 * Renders the app in dev "demo mode" (canned data, no backend) with a local
 * Chromium-based browser. Start the dev server first (`pnpm dev`), then:
 *
 *   pnpm screenshots
 *
 * Env: CHROME_PATH (browser executable), SCREENSHOT_URL (default
 * http://localhost:1420).
 */
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import puppeteer from "puppeteer-core";

const BASE_URL = process.env.SCREENSHOT_URL ?? "http://localhost:1420";
const OUT_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "docs", "screenshots");
const VIEWPORT = { width: 1440, height: 900, deviceScaleFactor: 2 };

const BROWSER_CANDIDATES = [
  process.env.CHROME_PATH,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Chromium.app/Contents/MacOS/Chromium",
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
];

/** Each shot: route (hash), demo query, what to wait for, optional action. */
const SHOTS = [
  { name: "overview", route: "#/", query: "theme=dark", waitFor: "text/Recent activity" },
  { name: "overview-light", route: "#/", query: "theme=light", waitFor: "text/Recent activity" },
  {
    name: "explorer",
    route: "#/connections/c-aws/buckets/media-originals?prefix=marketing/",
    query: "theme=dark&select=marketing/Q3-board-deck.pdf",
    waitFor: "text/application/pdf",
  },
  {
    name: "connection",
    route: "#/connections/c-aws",
    query: "theme=dark",
    waitFor: "text/media-originals",
  },
  {
    name: "add-connection",
    route: "#/",
    query: "theme=dark",
    waitFor: "text/Recent activity",
    action: async (page) => {
      await page.locator("::-p-text(Add connection)").click();
      await page.waitForSelector("::-p-text(Test connection)");
    },
  },
  {
    name: "settings",
    route: "#/settings",
    query: "theme=dark",
    waitFor: "text/Concurrent transfers",
  },
];

function findBrowser() {
  const path = BROWSER_CANDIDATES.find((candidate) => candidate && existsSync(candidate));
  if (!path) {
    throw new Error("No Chromium-based browser found. Set CHROME_PATH to its executable.");
  }
  return path;
}

async function main() {
  const reachable = await fetch(BASE_URL)
    .then((r) => r.ok)
    .catch(() => false);
  if (!reachable) {
    throw new Error(`Dev server not reachable at ${BASE_URL}. Start it with \`pnpm dev\`.`);
  }

  mkdirSync(OUT_DIR, { recursive: true });
  const browser = await puppeteer.launch({
    executablePath: findBrowser(),
    headless: true,
    defaultViewport: VIEWPORT,
    args: ["--hide-scrollbars", "--no-first-run", "--no-default-browser-check"],
  });

  try {
    for (const shot of SHOTS) {
      const page = await browser.newPage();
      await page.goto(`${BASE_URL}/?demo&${shot.query}${shot.route}`, { waitUntil: "load" });
      const [kind, value] = shot.waitFor.split(/\/(.*)/s);
      await page.waitForSelector(kind === "text" ? `::-p-text(${value})` : value, {
        timeout: 15000,
      });
      if (shot.action) await shot.action(page);
      // Let transitions and fonts settle.
      await page.evaluate(() => document.fonts.ready);
      await new Promise((resolve) => setTimeout(resolve, 600));
      const file = join(OUT_DIR, `${shot.name}.png`);
      await page.screenshot({ path: file });
      console.log(`✓ ${shot.name}.png`);
      await page.close();
    }
  } finally {
    await browser.close();
  }
}

main().catch((err) => {
  console.error(err.message ?? err);
  process.exit(1);
});
