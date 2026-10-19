/**
 * Renders docs/social/banner.html to PNG images for social media:
 *
 *   node scripts/social-images.mjs
 *
 * Outputs docs/social/bucklane-banner-wide.png (1280×640, also usable as the
 * GitHub social preview) and bucklane-banner-square.png (1080×1080), both at 2×.
 * Run `pnpm screenshots` first if the app screenshots are out of date.
 * Env: CHROME_PATH (browser executable).
 */
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import puppeteer from "puppeteer-core";

const SOCIAL_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "docs", "social");
const SIZES = [
  { name: "wide", width: 1280, height: 640 },
  { name: "square", width: 1080, height: 1080 },
];

const BROWSER_CANDIDATES = [
  process.env.CHROME_PATH,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Chromium.app/Contents/MacOS/Chromium",
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
];

const executablePath = BROWSER_CANDIDATES.find((p) => p && existsSync(p));
if (!executablePath) {
  console.error("No Chromium-based browser found. Set CHROME_PATH.");
  process.exit(1);
}

const browser = await puppeteer.launch({
  executablePath,
  // file:// pages need this to load the sibling images and fonts.
  args: ["--allow-file-access-from-files"],
});
try {
  const page = await browser.newPage();
  for (const { name, width, height } of SIZES) {
    await page.setViewport({ width, height, deviceScaleFactor: 2 });
    const url = pathToFileURL(join(SOCIAL_DIR, "banner.html"));
    url.search = `size=${name}`;
    await page.goto(url.href, { waitUntil: "networkidle0" });
    await page.evaluate(() => document.fonts.ready);
    const path = join(SOCIAL_DIR, `bucklane-banner-${name}.png`);
    await page.screenshot({ path, clip: { x: 0, y: 0, width, height } });
    console.log(`✓ ${path}`);
  }
} finally {
  await browser.close();
}
