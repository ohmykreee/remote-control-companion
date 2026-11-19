// Development helper: call the injected companion API in the remote WebView and
// save the returned PNG. Usage: node scripts/capture-vr-check.mjs [out.png] [port]
import fs from 'node:fs';
import path from 'node:path';

const out = process.argv[2] || '.scratch/vr-shot.png';
const port = Number(process.argv[3] || 9223);
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((target) => target.type === 'page');
if (!page) throw new Error('no remote page target');
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
const result = await send('Runtime.evaluate', {
  expression: `(async () => {
    const api = window.__DSH_COMPANION__;
    if (!api) return JSON.stringify({ ok: false, error: 'companion API missing' });
    try {
      const blob = await api.captureVrScreenshot();
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let binary = '';
      for (let i = 0; i < bytes.length; i += 0x8000) {
        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      }
      return JSON.stringify({ ok: true, type: blob.type, size: blob.size, data: btoa(binary) });
    } catch (error) {
      return JSON.stringify({ ok: false, error: error.message });
    }
  })()`,
  awaitPromise: true,
  returnByValue: true,
});
const raw = result.result?.result?.value;
if (!raw) throw new Error(JSON.stringify(result));
const value = JSON.parse(raw);
if (!value.ok) throw new Error(value.error);
fs.mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
fs.writeFileSync(out, Buffer.from(value.data, 'base64'));
console.log(JSON.stringify({ ok: true, type: value.type, size: value.size, out }));
ws.close();
