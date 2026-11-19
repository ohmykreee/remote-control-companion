// Development helper: collect console and exception events after a reload.
// Usage: node scripts/cdp-console.mjs [port] [waitMs]
const port = Number(process.argv[2] || 9222);
const waitMs = Number(process.argv[3] || 5000);
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((target) => target.type === 'page');
if (!page) throw new Error('no page target');
const ws = new WebSocket(page.webSocketDebuggerUrl);
let nextId = 1;
const pending = new Map();
ws.addEventListener('message', (event) => {
  const data = JSON.parse(event.data);
  if (data.id) {
    const resolve = pending.get(data.id);
    if (resolve) {
      pending.delete(data.id);
      resolve(data);
    }
    return;
  }
  if (data.method === 'Runtime.consoleAPICalled') {
    console.log(
      '[console]',
      data.params.type,
      data.params.args.map((arg) => arg.value ?? arg.description ?? arg.type).join(' '),
    );
  } else if (data.method === 'Runtime.exceptionThrown') {
    console.log(
      '[exception]',
      data.params.exceptionDetails.text,
      data.params.exceptionDetails.exception?.description ?? '',
    );
  } else if (data.method === 'Log.entryAdded') {
    console.log('[log]', data.params.entry.level, data.params.entry.text);
  }
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
await send('Runtime.enable');
await send('Log.enable');
await send('Page.enable');
await send('Page.reload', { ignoreCache: true });
await new Promise((resolve) => setTimeout(resolve, waitMs));
const state = await send('Runtime.evaluate', {
  expression: 'JSON.stringify({text: document.body.innerText, html: document.body.innerHTML.slice(0, 800)})',
  returnByValue: true,
});
console.log(state.result?.result?.value);
ws.close();
