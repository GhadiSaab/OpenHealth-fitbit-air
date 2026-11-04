// End-to-end smoke test for the PWA build (app/dist).
// Usage: node scripts/e2e-smoke.cjs [screenshot-dir]
// Serves app/dist on a local port, drives the app in headless Chromium and
// fails on console errors or missing UI.
const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', 'app', 'dist');
const shots = process.argv[2];
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json', '.webmanifest': 'application/manifest+json', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2' };
const server = http.createServer((req, res) => {
  const p = path.join(root, decodeURIComponent(req.url.split('?')[0]).replace(/^\/+/, '') || 'index.html');
  fs.readFile(fs.existsSync(p) && fs.statSync(p).isFile() ? p : path.join(root, 'index.html'), (err, data) => {
    if (err) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { 'content-type': types[path.extname(p)] || 'application/octet-stream' });
    res.end(data);
  });
});

function check(cond, msg) { if (!cond) { throw new Error('FAIL: ' + msg); } console.log('ok -', msg); }

(async () => {
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1, timezoneId: 'Europe/Berlin' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', (e) => errors.push(String(e)));
  const shot = async (name) => { if (shots) await page.screenshot({ path: path.join(shots, name + '.png'), fullPage: true }); };

  await page.goto(base);
  await page.getByRole('heading', { name: 'Airspace' }).waitFor({ timeout: 20000 });
  check(true, 'onboarding renders');
  await shot('01-onboarding');

  // Empty database path first: "Set up my device later" → empty Today.
  await page.getByRole('button', { name: /Set up my device later/ }).click();
  await page.getByText('No data yet').waitFor();
  check(true, 'empty database shows a friendly empty state');
  await shot('02-empty-today');

  // Explore the demo.
  await page.getByRole('button', { name: 'Explore demo' }).click();
  await page.getByText(/Demo data\./).first().waitFor({ timeout: 30000 });
  await page.getByText(/Steps/).first().waitFor();
  check(await page.getByText(/disconnected/).count() > 0, 'demo shows a disconnected device state');
  check(await page.getByText(/Last reading/).count() > 0, 'stale live reading is labelled "Last reading"');
  await shot('03-today-demo');

  // Persistence across reload.
  await page.reload();
  await page.getByText(/Demo data\./).first().waitFor({ timeout: 30000 });
  check(true, 'demo database and route persist across reload');

  // Explore and a metric detail.
  await page.getByRole('button', { name: 'Explore', exact: true }).click();
  await page.getByRole('heading', { name: 'Explore', exact: true }).waitFor();
  await shot('04-explore');
  await page.getByRole('button', { name: /^Heart rate/ }).first().click();
  await page.getByRole('img', { name: /Heart rate on/ }).waitFor();
  check(await page.getByText(/samples, shown as/).count() === 1, 'intraday heart-rate chart with sample summary');
  await shot('05-heart-day');

  // Keyboard inspection of a chart.
  await page.getByRole('radio', { name: 'Week' }).click();
  const chart = page.getByRole('img', { name: /Heart rate for/ });
  await chart.focus();
  await page.keyboard.press('Home');
  const readout = await page.locator('.chart-readout').first().innerText();
  check(readout.length > 0, 'keyboard inspection updates the readout: ' + readout.replace(/\n/g, ' '));
  await page.getByRole('button', { name: /Show table/ }).click();
  check(await page.locator('table.data-table').count() > 0, 'table alternative is available');
  await shot('06-heart-week');

  // Sleep day view (stage timeline).
  await page.goto(base + '#/explore/sleep/d');
  await page.getByRole('img', { name: /Sleep stages/ }).waitFor({ timeout: 15000 });
  check(true, 'sleep stage timeline renders');
  await shot('07-sleep-day');

  await page.goto(base + '#/explore/skin_temperature/m');
  await page.getByText(/not core body temperature/).first().waitFor();
  check(true, 'skin temperature is labelled as a deviation, not core temperature');
  await shot('08-skin-temp');

  // Journal.
  await page.getByRole('button', { name: 'Journal', exact: true }).click();
  await page.getByRole('heading', { name: 'Journal', exact: true }).waitFor();
  await page.getByPlaceholder(/How was the day/).fill('Smoke test note');
  await page.getByRole('button', { name: 'Save entry' }).click();
  await page.getByText('Journal saved').waitFor();
  check(true, 'journal entry saves');
  await shot('09-journal');

  // Coach without AI: local insights only, no network.
  await page.getByRole('button', { name: 'Coach', exact: true }).click();
  await page.getByRole('heading', { name: 'Insights' }).waitFor();
  check(await page.getByRole('button', { name: 'Set up AI coach' }).count() === 1, 'AI is optional and not configured by default');
  check(await page.locator('.insight').count() > 0, 'local insights render without AI');
  await shot('10-coach');

  // Data screen.
  await page.goto(base + '#/profile/data');
  await page.getByRole('heading', { name: 'Data', exact: true }).waitFor();
  await shot('11-data');

  // Dark theme.
  await page.goto(base + '#/profile/settings');
  await page.getByRole('radio', { name: 'Dark' }).click();
  await page.goto(base + '#/today');
  await page.getByText(/Steps/).first().waitFor();
  check(await page.evaluate(() => document.documentElement.dataset.theme) === 'dark', 'dark theme applies');
  await shot('12-today-dark');

  // Desktop layout.
  await page.setViewportSize({ width: 1280, height: 900 });
  await shot('13-desktop-dark');

  const bad = errors.filter((e) => !/favicon/.test(e));
  check(bad.length === 0, 'no console errors' + (bad.length ? ': ' + bad.join(' | ') : ''));
  await browser.close();
  server.close();
  console.log('\nE2E smoke test passed');
})().catch(async (e) => { console.error(e.message || e); process.exit(1); });
