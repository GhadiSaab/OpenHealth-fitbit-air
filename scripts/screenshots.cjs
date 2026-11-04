// Captures the README screenshots from the PWA build (app/dist) using the demo
// dataset and a local mock AI provider. Usage: node scripts/screenshots.cjs
const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', 'app', 'dist');
const out = path.join(__dirname, '..', 'docs', 'screenshots');
fs.mkdirSync(out, { recursive: true });
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.webmanifest': 'application/manifest+json', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2' };
const server = http.createServer((req, res) => {
  const p = path.join(root, decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html');
  fs.readFile(fs.existsSync(p) && fs.statSync(p).isFile() ? p : path.join(root, 'index.html'), (err, data) => {
    res.writeHead(err ? 404 : 200, { 'content-type': types[path.extname(p)] || 'application/octet-stream' });
    res.end(data);
  });
});
const mock = http.createServer((req, res) => {
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*', 'access-control-allow-methods': 'GET, POST, OPTIONS' };
  if (req.method === 'OPTIONS') { res.writeHead(204, cors); return res.end(); }
  req.resume();
  req.on('end', () => {
    res.writeHead(200, { ...cors, 'content-type': 'application/json' });
    if (req.url === '/v1/models') return res.end('{"data":[]}');
    res.end(JSON.stringify({
      stop_reason: 'end_turn',
      content: [{ type: 'text', text: 'Your steps averaged about 8,600 a day over the last week, with your highest days on Monday and Wednesday (the days you tagged exercise).\n\nSleep was steadier: most nights you fell asleep between 23:00 and 23:40 and slept around 7 h 20 min. Two nights after late caffeine started later.\n\nOne modest idea: keep caffeine before early afternoon on workdays and see whether bedtimes become more consistent. (Example reply from a local mock provider used for this screenshot.)' }],
    }));
  });
});

(async () => {
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  await new Promise((r) => mock.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${server.address().port}/`;
  const mockUrl = `http://127.0.0.1:${mock.address().port}`;
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, timezoneId: 'Europe/Berlin', colorScheme: 'light' });
  const page = await ctx.newPage();
  const settle = () => page.waitForTimeout(450);
  const shot = async (name, opts = {}) => { await settle(); await page.screenshot({ path: path.join(out, name + '.png'), ...opts }); console.log('saved', name); };
  const go = async (hash, waitText) => { await page.goto(base + hash); if (waitText) await page.getByText(waitText).first().waitFor({ timeout: 20000 }); };

  await page.goto(base);
  await page.getByRole('heading', { name: 'Airspace' }).waitFor({ timeout: 20000 });
  await shot('01-onboarding');
  await page.getByRole('button', { name: /Explore the demo/ }).click();
  await page.getByText(/Demo data\./).first().waitFor({ timeout: 30000 });
  await page.getByText(/Steps/).first().waitFor();
  await shot('02-today');
  await page.evaluate(() => window.scrollTo(0, 820));
  await shot('03-today-trends');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.getByRole('button', { name: /Demo device/ }).click();
  await page.getByRole('heading', { name: 'About these readings' }).waitFor();
  await shot('04-status-sheet');

  await go('#/explore', 'Explore');
  await shot('05-explore');
  await go('#/explore/heart_rate/d', 'samples, shown as');
  await shot('06-heart-rate-day');
  await go('#/explore/steps/m', 'Calendar');
  await page.getByRole('button', { name: 'Previous period' }).click();
  await page.getByText('October 2026').first().waitFor();
  await shot('07-steps-month');
  await page.getByRole('heading', { name: 'Calendar' }).scrollIntoViewIfNeeded();
  await page.evaluate(() => window.scrollBy(0, -80));
  await shot('08-steps-calendar');
  // Back to today (the month navigation moved the selected day).
  await go('#/today', 'Steps');
  const chip = page.getByRole('button', { name: 'Today', exact: true }).filter({ hasNot: page.locator('svg') });
  if (await chip.count()) await chip.first().click();
  await go('#/explore/sleep/d', 'Stage intervals as reported');
  await shot('09-sleep-day');
  await go('#/explore/hrv/m', 'Measurement method');
  await shot('10-hrv-month');
  await go('#/explore/skin_temperature/m', 'Reading this chart');
  await shot('11-skin-temperature');
  await go('#/explore/heart_rate/w');
  await page.getByRole('button', { name: /^Details for/ }).click();
  await page.getByRole('button', { name: 'Show individual records' }).click();
  await page.getByText('Stored records').first().waitFor();
  await page.locator('.records').scrollIntoViewIfNeeded();
  await shot('12-provenance-sheet');
  await page.keyboard.press('Escape');

  await go('#/journal', 'Entry');
  await shot('13-journal');
  await page.getByRole('heading', { name: 'Tags and your measurements' }).scrollIntoViewIfNeeded();
  await shot('14-journal-associations');

  await go('#/coach', 'Insights');
  await shot('15-coach-insights');
  await go('#/coach/settings', 'Provider');
  await page.getByLabel('HTTPS endpoint').fill(mockUrl);
  await page.getByPlaceholder('Paste your key').fill('sk-demo-not-a-real-key');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByText('Saved.').waitFor();
  await page.getByLabel('HTTPS endpoint').fill('https://api.anthropic.com');
  await shot('16-ai-settings');
  await page.getByLabel('HTTPS endpoint').fill(mockUrl);
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await go('#/coach', 'Insights');
  await page.getByRole('button', { name: 'Weekly review' }).click();
  await page.getByRole('heading', { name: 'Review before sending' }).waitFor();
  await page.locator('.consent').evaluate((el) => { el.scrollIntoView({ block: 'start' }); window.scrollBy(0, -16); });
  await shot('17-ai-consent');
  await page.getByRole('button', { name: 'Share and ask' }).click();
  await page.getByText(/Example reply from a local mock provider/).waitFor();
  await page.locator('.ai-answer').scrollIntoViewIfNeeded();
  await page.evaluate(() => window.scrollBy(0, -120));
  await shot('18-ai-answer');

  await go('#/profile', 'Device');
  await shot('19-profile');
  await go('#/profile/device', 'States you may see');
  await shot('20-device');
  await go('#/profile/data', 'Active data');
  await shot('21-data');
  await go('#/profile/settings', 'Appearance');
  await shot('23-settings');

  // Dark theme, phone and desktop.
  await page.getByRole('radio', { name: 'Dark' }).click();
  await go('#/today', 'Steps');
  await shot('24-today-dark');
  await go('#/explore/heart_rate/d', 'samples, shown as');
  await shot('25-heart-rate-dark');
  await page.setViewportSize({ width: 1280, height: 860 });
  await go('#/today', 'Steps');
  await shot('26-desktop-dark');
  await go('#/profile/settings', 'Appearance');
  await page.getByRole('radio', { name: 'Light' }).click();
  await go('#/explore/steps/m', 'Calendar');
  await shot('27-desktop-steps');

  // Import preview (switches to the personal database, so it runs last).
  await page.setViewportSize({ width: 390, height: 844 });
  await go('#/profile/data/import', 'Choose a file');
  await page.locator('input[type=file]').setInputFiles(path.join(__dirname, '..', 'examples', 'sample-import.json'));
  await page.getByRole('heading', { name: 'Preview' }).waitFor({ timeout: 20000 });
  await page.getByRole('heading', { name: 'Preview' }).scrollIntoViewIfNeeded();
  await page.locator('.toast').waitFor({ state: 'detached', timeout: 10000 }).catch(() => {});
  await shot('22-import-preview');

  // Empty database.
  const fresh = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, timezoneId: 'Europe/Berlin' });
  const p2 = await fresh.newPage();
  await p2.goto(base);
  await p2.getByRole('button', { name: /Set up my device later/ }).click();
  await p2.getByRole('heading', { name: 'No data yet' }).waitFor();
  await p2.waitForTimeout(400);
  await p2.screenshot({ path: path.join(out, '28-empty.png') });
  console.log('saved 28-empty');

  await browser.close();
  server.close();
  mock.close();
})().catch((e) => { console.error(e); process.exit(1); });
