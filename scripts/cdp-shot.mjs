// Development helper: screenshot a WebView2 target exposed by the debug port.
// Usage: node scripts/cdp-shot.mjs <out.png> [port]
import fs from 'node:fs';
import path from 'node:path';

const out = process.argv[2] || 'scratch/companion.png';
const port = Number(process.argv[3] || 9222);
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((target) => target.type === 'page');
if (!page) throw new Error('no page target');

const ws = new WebSocket(page.webSocketDebuggerUrl);
let nextId = 1;
const pending = new Map();
ws.addEventListener('message', (event) => {
  const data = JSON.parse(event.data);
  if (!data.id) return;
  const resolve = pending.get(data.id);
  if (!resolve) return;
  pending.delete(data.id);
  resolve(data);
});
await new Promise((resolve, reject) => {
  ws.addEventListener('open', resolve, { once: true });
  ws.addEventListener('error', reject, { once: true });
});

function send(method, params = {}) {
  const id = nextId++;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve) => pending.set(id, resolve));
}

const info = await send('Runtime.evaluate', {
  expression: 'JSON.stringify({href: location.href, title: document.title, dpr: devicePixelRatio})',
  returnByValue: true,
});
console.log(info.result?.result?.value);
const shot = await send('Page.captureScreenshot', { format: 'png', fromSurface: true });
if (!shot.result?.data) throw new Error(JSON.stringify(shot));
fs.mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
fs.writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
console.log(`wrote ${out}`);
ws.close();
