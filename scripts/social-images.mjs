/**
 * Renders the social banners in docs/social to PNG images:
 *
 *   node scripts/social-images.mjs
 *
 * banner.html → bucklane-banner-{wide,square}.png (the wide one doubles as the
 * GitHub social preview); update.html → bucklane-update-{wide,square}.png
 * for release announcements. Wide is 1280×640, square 1080×1080, both at 2×.
 * Run `pnpm screenshots` first if the app screenshots are out of date.
 * Env: CHROME_PATH (browser executable).
 */
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import puppeteer from "puppeteer-core";

const SOCIAL_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "docs", "social");
const PAGES = [
  { file: "banner.html", output: "bucklane-banner" },
  { file: "update.html", output: "bucklane-update" },
];
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
  for (const { file, output } of PAGES) {
    for (const { name, width, height } of SIZES) {
      await page.setViewport({ width, height, deviceScaleFactor: 2 });
      const url = pathToFileURL(join(SOCIAL_DIR, file));
      url.search = `size=${name}`;
      await page.goto(url.href, { waitUntil: "networkidle0" });
      await page.evaluate(() => document.fonts.ready);
      const path = join(SOCIAL_DIR, `${output}-${name}.png`);
      await page.screenshot({ path, clip: { x: 0, y: 0, width, height } });
      console.log(`✓ ${path}`);
    }
  }
} finally {
  await browser.close();
}
