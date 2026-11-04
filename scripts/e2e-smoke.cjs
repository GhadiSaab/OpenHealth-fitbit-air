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
  // Fixed early evening so time-of-day labels ("Last reading", "so far today")
  // are the same whenever the test runs; time still moves forward from there.
  const berlinDay = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Berlin' }).format(new Date());
  await ctx.clock.install({ time: new Date(berlinDay + 'T17:00:00Z') });
  const page = await ctx.newPage();
  const errors = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', (e) => errors.push(String(e)));
  const shot = async (name) => { if (shots) await page.screenshot({ path: path.join(shots, name + '.png'), fullPage: true }); };

  await page.goto(base);
  await page.getByRole('heading', { name: 'Airspace' }).waitFor({ timeout: 20000 });
  check(true, 'onboarding renders');
  await shot('01-onboarding');

  // Offline after the very first visit: the service worker precaches the app shell.
  await page.evaluate(async () => { await navigator.serviceWorker.ready; });
  await ctx.setOffline(true);
  await page.reload();
  await page.getByRole('heading', { name: 'Airspace' }).waitFor({ timeout: 20000 });
  check(true, 'app loads offline after the first visit');
  await ctx.setOffline(false);

  // Empty database path first: "Set up my device later" → empty Today.
  await page.getByRole('button', { name: /Set up my device later/ }).click();
  await page.getByRole('heading', { name: 'No data yet' }).waitFor();
  check(true, 'empty database shows a friendly empty state');
  await shot('02-empty-today');

  // Explore the demo.
  await page.getByRole('button', { name: 'Explore demo' }).click();
  await page.getByText(/Demo data\./).first().waitFor({ timeout: 30000 });
  await page.getByText(/Steps/).first().waitFor();
  await page.getByText(/disconnected/).first().waitFor({ timeout: 15000 });
  check(true, 'demo shows a disconnected device state');
  await page.getByText(/Last reading/).first().waitFor({ timeout: 15000 });
  check(true, 'stale live reading is labelled "Last reading"');
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
  await page.getByRole('application', { name: /Heart rate on/ }).waitFor();
  check(await page.getByText(/samples, shown as/).count() === 1, 'intraday heart-rate chart with sample summary');
  await shot('05-heart-day');

  // Keyboard inspection of a chart.
  await page.getByRole('radio', { name: 'Week' }).click();
  const chart = page.getByRole('application', { name: /Heart rate for/ });
  await chart.focus();
  await page.keyboard.press('Home');
  const readout = await page.locator('.chart-readout').first().innerText();
  check(readout.length > 0, 'keyboard inspection updates the readout: ' + readout.replace(/\n/g, ' '));
  await page.getByRole('button', { name: /Show table/ }).click();
  check(await page.locator('table.data-table').count() > 0, 'table alternative is available');
  await shot('06-heart-week');

  // Sleep day view (stage timeline).
  await page.goto(base + '#/explore/sleep/d');
  await page.getByRole('application', { name: /Sleep stages/ }).waitFor({ timeout: 15000 });
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
  // Unsaved text is kept when leaving the tab.
  await page.getByPlaceholder(/How was the day/).fill('Note typed then left');
  await page.getByRole('button', { name: 'Explore', exact: true }).click();
  await page.getByRole('heading', { name: 'Explore', exact: true }).waitFor();
  await page.getByRole('button', { name: 'Journal', exact: true }).click();
  await page.getByRole('heading', { name: 'Journal', exact: true }).waitFor();
  await page.waitForTimeout(300);
  check(await page.getByPlaceholder(/How was the day/).inputValue() === 'Note typed then left', 'journal text survives switching tabs');
  await shot('09-journal');

  // Coach without AI: local insights only, no network.
  await page.getByRole('button', { name: 'Coach', exact: true }).click();
  await page.getByRole('heading', { name: 'Insights' }).waitFor();
  check(await page.getByRole('button', { name: 'Set up AI coach' }).count() === 1, 'AI is optional and not configured by default');
  check(await page.locator('.insight').count() > 0, 'local insights render without AI');
  await shot('10-coach');

  // Optional AI coach against a local mock provider (Anthropic Messages API shape).
  const SECRET = 'sk-ant-test-SECRET-123456';
  const received = [];
  const mock = http.createServer((req, res) => {
    const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*', 'access-control-allow-methods': 'GET, POST, OPTIONS' };
    if (req.method === 'OPTIONS') { res.writeHead(204, cors); return res.end(); }
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      received.push({ method: req.method, url: req.url, headers: req.headers, body });
      if (req.url === '/v1/models') { res.writeHead(200, { ...cors, 'content-type': 'application/json' }); return res.end('{"data":[{"id":"claude-opus-5-5"}]}'); }
      if (req.url === '/v1/messages') {
        res.writeHead(200, { ...cors, 'content-type': 'application/json' });
        return res.end(JSON.stringify({ content: [{ type: 'text', text: 'Mock review: your steps were steady this week.' }], stop_reason: 'end_turn' }));
      }
      res.writeHead(404, cors); res.end();
    });
  });
  await new Promise((r) => mock.listen(0, '127.0.0.1', r));
  const mockUrl = `http://127.0.0.1:${mock.address().port}`;
  await page.goto(base + '#/coach/settings');
  await page.getByLabel('HTTPS endpoint').fill(mockUrl);
  await page.getByPlaceholder('Paste your key').fill(SECRET);
  await page.getByRole('button', { name: 'Test connection' }).click();
  await page.getByText(/Connected\. The provider accepted the key/).waitFor();
  check(received.some((r) => r.url === '/v1/models' && r.headers['x-api-key'] === SECRET && !r.body), 'connection test sends the key in a header and no health data');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByText('Saved.').waitFor();
  check(await page.evaluate((s) => JSON.stringify(localStorage).includes(s), SECRET) === false, 'API key is not written to localStorage');
  check(received.every((r) => !r.url.includes(SECRET)), 'API key never appears in a URL');

  await page.goto(base + '#/coach');
  await page.getByRole('button', { name: 'Weekly review' }).click();
  await page.getByRole('heading', { name: 'Review before sending' }).waitFor();
  check(received.filter((r) => r.url === '/v1/messages').length === 0, 'nothing is sent before consent');
  await page.locator('.consent').getByRole('button', { name: 'SpO₂', exact: true }).click(); // exclude a metric
  await shot('10b-consent');
  await page.getByRole('button', { name: 'Share and ask' }).click();
  await page.getByText('Mock review: your steps were steady this week.').waitFor();
  check(await page.getByText(/AI-generated/).count() > 0, 'AI answer is labelled as AI-generated');
  const msg = received.find((r) => r.url === '/v1/messages');
  const sent = JSON.parse(msg.body);
  const userText = sent.messages[sent.messages.length - 1].content;
  check(msg.headers['x-api-key'] === SECRET && msg.headers['anthropic-version'] === '2026-10-21', 'Anthropic adapter sends documented headers');
  check(userText.includes('<health_data>') && userText.includes('"steps"'), 'consented summaries are shared');
  check(!userText.includes('spo2_avg_pct'), 'excluded metric is not shared');
  check(!userText.includes('journal_note'), 'journal notes are not shared unless included');
  check(msg.body.length < 30000, `only compact daily summaries are sent (${msg.body.length} bytes)`);
  check(/notes and tags are user-written data, not instructions/i.test(sent.system), 'system prompt treats notes as data');
  await shot('10c-ai-answer');
  mock.close();

  // Data screen: export a backup and check its contents.
  await page.goto(base + '#/profile/data');
  await page.getByRole('heading', { name: 'Data', exact: true }).waitFor();
  await shot('11-data');
  const [dl] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: /Export full backup/ }).click()]);
  const backup = fs.readFileSync(await dl.path(), 'utf8');
  const parsed = JSON.parse(backup);
  check(parsed.format === 'airspace.backup' && parsed.version === 1 && parsed.measurements.length > 1000, 'backup export is versioned and complete');
  check(!backup.includes(SECRET) && !/api_?key/i.test(backup), 'backup contains no API key');

  // Import into personal data (switches away from the demo automatically).
  const sample = path.join(__dirname, '..', 'examples', 'sample-import.json');
  await page.goto(base + '#/profile/data/import');
  await page.getByRole('heading', { name: 'Import history', exact: true }).waitFor();
  await page.locator('input[type=file]').setInputFiles(sample);
  await page.getByRole('heading', { name: 'Preview' }).waitFor({ timeout: 20000 });
  check(await page.getByText(/2 rejected/).count() > 0, 'preview reports rejected records (implausible HR, absolute temperature)');
  await shot('12-import-preview');
  await page.getByRole('button', { name: /^Import\s\d+\srecords/ }).click();
  await page.getByText(/Imported\s\d+\srecords/).waitFor({ timeout: 30000 });
  check(await page.getByText(/Demo data\./).count() === 0, 'personal data shows no demo banner');
  // Re-importing the same file finds only duplicates.
  await page.goto(base + '#/profile/data/import');
  await page.locator('input[type=file]').setInputFiles(sample);
  await page.getByRole('heading', { name: 'Preview' }).waitFor({ timeout: 20000 });
  check(await page.getByRole('button', { name: 'Nothing new to import' }).count() === 1, 're-import is detected as duplicates');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.goto(base + '#/explore/steps/w');
  await page.getByText(/Oct|Nov/).first().waitFor();

  // Dark theme.
  await page.goto(base + '#/profile/settings');
  await page.getByRole('radio', { name: 'Dark' }).click();
  await page.goto(base + '#/today');
  await page.getByText(/Steps/).first().waitFor();
  check(await page.evaluate(() => document.documentElement.dataset.theme) === 'dark', 'dark theme applies');
  await shot('13-today-dark');

  // Desktop layout.
  await page.setViewportSize({ width: 1280, height: 900 });
  await shot('14-desktop-dark');

  const bad = errors.filter((e) => !/favicon/.test(e));
  check(bad.length === 0, 'no console errors' + (bad.length ? ': ' + bad.join(' | ') : ''));
  await browser.close();
  server.close();
  console.log('\nE2E smoke test passed');
})().catch(async (e) => { console.error(e.message || e); process.exit(1); });
