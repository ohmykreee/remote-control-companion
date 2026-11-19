import http from 'node:http';

const PORT = Number(process.env.MOCK_REMOTE_PORT || 39001);

const page = String.raw`<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Mock 远控页面</title>
<style>
  :root { color-scheme: dark; font-family: "Segoe UI", "Microsoft YaHei UI", sans-serif; }
  * { box-sizing: border-box; }
  body { margin: 0; min-height: 100vh; background: radial-gradient(800px 500px at 20% -10%, #1c315f, #090d14 65%); color: #e8eef9; }
  main { max-width: 920px; margin: 0 auto; padding: 56px 36px 72px; }
  .eyebrow { color: #7f9cff; font-size: 11px; letter-spacing: .16em; font-family: monospace; }
  h1 { margin: 14px 0 10px; font-size: 38px; letter-spacing: -.035em; }
  p { color: #8d9bb0; line-height: 1.7; font-size: 14px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 14px; margin: 28px 0; }
  .card { border: 1px solid rgb(255 255 255 / 9%); border-radius: 14px; background: rgb(15 22 34 / 80%); padding: 18px; }
  .label { color: #6f7e96; font-size: 11px; text-transform: uppercase; letter-spacing: .08em; margin-bottom: 8px; }
  .value { font-size: 15px; font-family: "Cascadia Code", monospace; color: #dce6ff; word-break: break-all; }
  .ok { color: #6ee7be; } .bad { color: #f29baa; }
  .actions { display: flex; gap: 10px; flex-wrap: wrap; margin: 22px 0; }
  button { border: 1px solid rgb(255 255 255 / 12%); border-radius: 9px; background: #1a2332; color: #e5ecf8; padding: 9px 14px; font-size: 13px; cursor: pointer; }
  button:hover { background: #233148; }
  button.primary { background: linear-gradient(180deg, #5d7dff, #4262e8); border-color: #6e8bff; }
  pre { white-space: pre-wrap; background: #070b11; border: 1px solid rgb(255 255 255 / 8%); border-radius: 10px; padding: 14px; color: #91a1b8; font-size: 12px; min-height: 80px; }
  #shot { display: none; max-width: 100%; border-radius: 12px; border: 1px solid rgb(255 255 255 / 12%); margin-top: 16px; }
  .path { color: #59677d; font-size: 12px; margin-top: 32px; }
</style>
</head>
<body>
<main>
  <div class="eyebrow">MOCK DSH REMOTE CONTROL</div>
  <h1>远控页面已连接</h1>
  <p>这个页面用于验证伴侣窗口的远控加载、权限、麦克风选择和 VR 截图接口。它模拟 dsh-remote-control 的页面头部，并禁用了 iframe 嵌入。</p>
  <div class="grid">
    <div class="card"><div class="label">安全上下文</div><div id="secure" class="value">检测中</div></div>
    <div class="card"><div class="label">伴侣截图 API</div><div id="api" class="value">检测中</div></div>
    <div class="card"><div class="label">麦克风</div><div id="mic" class="value">未请求</div></div>
  </div>
  <div class="actions">
    <button class="primary" id="capture">获取 VR 截图</button>
    <button id="askmic">请求麦克风</button>
    <button id="reload">重新载入</button>
  </div>
  <pre id="log">等待操作…</pre>
  <img id="shot" alt="VR 截图结果">
  <div class="path" id="origin"></div>
</main>
<script>
  const $ = (id) => document.getElementById(id);
  function log(value) {
    $('log').textContent = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
  }
  $('secure').textContent = window.isSecureContext ? '安全' : '不安全';
  $('secure').className = 'value ' + (window.isSecureContext ? 'ok' : 'bad');
  $('origin').textContent = location.origin + '  ·  ' + navigator.userAgent;

  function detectApi() {
    const api = window.__DSH_COMPANION__;
    $('api').textContent = api && typeof api.captureVrScreenshot === 'function' ? '可用' : '不可用';
    $('api').className = 'value ' + (api && api.captureVrScreenshot ? 'ok' : 'bad');
    return api;
  }
  detectApi();
  window.addEventListener('dsh-companion-ready', (event) => {
    log('伴侣接口已就绪: ' + JSON.stringify(event.detail));
    detectApi();
  });

  $('capture').addEventListener('click', async () => {
    const api = detectApi();
    if (!api) return log('伴侣截图 API 不可用');
    log('正在请求 VR 截图…');
    try {
      const blob = await api.captureVrScreenshot();
      const url = URL.createObjectURL(blob);
      $('shot').src = url;
      $('shot').style.display = 'block';
      log('截图成功: ' + blob.type + ', ' + blob.size + ' bytes');
    } catch (error) {
      log('截图失败: ' + (error && error.message ? error.message : String(error)));
    }
  });

  $('askmic').addEventListener('click', async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const track = stream.getAudioTracks()[0];
      $('mic').textContent = track ? track.label : '没有音轨';
      $('mic').className = 'value ok';
      log('麦克风已打开: ' + (track ? track.label : '未知'));
    } catch (error) {
      $('mic').textContent = '失败';
      $('mic').className = 'value bad';
      log('麦克风失败: ' + (error && error.message ? error.message : String(error)));
    }
  });

  $('reload').addEventListener('click', () => location.reload());
</script>
</body>
</html>`;

const server = http.createServer((req, res) => {
  if (req.url === '/favicon.ico') {
    res.writeHead(204);
    res.end();
    return;
  }
  res.writeHead(200, {
    'content-type': 'text/html; charset=utf-8',
    'cache-control': 'no-store',
    'content-security-policy':
      "default-src 'self'; img-src 'self' data: blob:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; frame-ancestors 'none'; connect-src 'self' ws: wss:",
  });
  res.end(page);
});

server.listen(PORT, '0.0.0.0', () => {
  console.log(`[mock-remote] http://127.0.0.1:${PORT}/`);
  console.log(`[mock-remote] frame-ancestors: none (top-level webview required)`);
});
