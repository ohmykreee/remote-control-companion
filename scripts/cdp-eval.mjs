// Development helper: evaluate JavaScript in the first WebView2 page target.
// Usage: node scripts/cdp-eval.mjs "<expression>" [port]
const expression = process.argv[2] || 'document.body.innerText';
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
const result = await send('Runtime.evaluate', {
  expression,
  awaitPromise: true,
  returnByValue: true,
  userGesture: true,
});
console.log(JSON.stringify(result.result ?? result, null, 2));
ws.close();
