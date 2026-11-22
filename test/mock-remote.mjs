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
<div id="diag" style="position:fixed;top:0;left:0;right:0;z-index:9999;background:rgba(0,0,0,.82);color:#9ff;font:12px monospace;padding:6px 10px;white-space:pre-wrap"></div>
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
    <button id="theme">切换主题</button>
    <button id="language">切换语言</button>
    <button id="reload">重新载入</button>
  </div>
  <pre id="log">等待操作…</pre>
  <img id="shot" alt="VR 截图结果">
  <div class="path" id="origin"></div>
</main>
<script>
  const $ = (id) => document.getElementById(id);
  const diag = (value) => {
    $('diag').textContent = String(value);
  };
  function log(value) {
    $('log').textContent = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
    diag(value);
  }
  $('secure').textContent = window.isSecureContext ? '安全' : '不安全';
  $('secure').className = 'value ' + (window.isSecureContext ? 'ok' : 'bad');
  $('origin').textContent = location.origin + '  ·  ' + navigator.userAgent;

  function detectApi() {
    const api = window.__DSH_COMPANION__;
    $('api').textContent = api && typeof api.captureVrScreenshot === 'function' ? '可用' : '不可用';
    $('api').className = 'value ' + (api && api.captureVrScreenshot ? 'ok' : 'bad');
    if (!diag.textContent) diag('API=' + (api && api.captureVrScreenshot ? 'available' : 'missing') + ' lang=' + document.documentElement.lang);
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

  const params = new URLSearchParams(location.search);
  let light = false;
  function applyTheme(value, announce = true) {
    light = value;
    document.body.dataset.theme = light ? 'light' : 'dark';
    document.documentElement.classList.toggle('light', light);
    document.documentElement.classList.toggle('dark', !light);
    document.documentElement.style.colorScheme = light ? 'light' : 'dark';
    document.body.style.background = light
      ? 'radial-gradient(800px 500px at 20% -10%, #e7effc, #f7f9fd 65%)'
      : 'radial-gradient(800px 500px at 20% -10%, #1c315f, #090d14 65%)';
    document.body.style.color = light ? '#162033' : '#e8eef9';
    if (announce) log('主题已切换: ' + (light ? 'light' : 'dark'));
  }
  applyTheme(params.get('theme') === 'light', false);
  $('theme').addEventListener('click', () => applyTheme(!light));

  let english = false;
  function applyLanguage(value, announce = true) {
    english = value;
    document.documentElement.lang = english ? 'en-US' : 'zh-CN';
    document.querySelector('h1').textContent = english ? 'Remote page connected' : '远控页面已连接';
    if (announce) log('语言已切换: ' + document.documentElement.lang);
  }
  applyLanguage(params.get('lang') === 'en', false);
  $('language').addEventListener('click', () => applyLanguage(!english));

  if (params.get('auto') === '1') {
    let tick = 0;
    setInterval(() => {
      tick += 1;
      applyTheme(tick % 2 === 1);
      applyLanguage(tick % 2 === 1);
    }, 2000);
  }
  if (params.get('capture') === '1') {
    setTimeout(() => {
      diag('API=' + (window.__DSH_COMPANION__ ? 'available' : 'missing') + '; requesting capture');
      $('capture').click();
    }, 2500);
  }

  // Fixture for the companion's VRChat OSC panel. The real page exposes the
  // same shape at window.__DSH_REMOTE__; the injected status reporter only
  // reads the fields it needs.
  const mockMedia = {
    title: 'Mock Song',
    artist: 'Mock Artist',
    album: 'Mock Album',
    playbackStatus: 'Playing',
    position: 0,
    startTime: 0,
    endTime: 200,
  };
  window.__DSH_REMOTE__ = {
    online: true,
    temperatureUnit: 'celsius',
    weather: {
      ok: true,
      location: { name: 'Mock City' },
      current: { code: 2, isDay: false, temperature: 14, feelsLike: 12, humidity: 71 },
      daily: { high: 15, low: 13 },
    },
    media: { media: { media: mockMedia }, volume: { volume: 0.6, muted: false } },
    performance: {
      cpu: { model: 'AMD Ryzen 7 5700X3D 8-Core Processor', usage: 42 },
      memory: { total: 34359738368, used: 13743895347, percent: 39 },
      gpu: {
        ok: true,
        name: 'AMD Radeon RX 7800 XT',
        utilization: 61,
        memoryTotal: 8589934592,
        memoryUsed: 4509715660,
        memoryPercent: 52,
      },
    },
    sessions: [{ id: 'mock-running', status: 'running' }, { id: 'mock-done', status: 'done' }],
    account: {
      available: true,
      signedIn: true,
      lowBalance: false,
      balance: { currency: 'CNY', balance: 12.34, state: 'ready' },
      usage: { currency: 'CNY', today: 3.21 },
    },
    clockNow: Date.now(),
  };
  setInterval(() => {
    mockMedia.position = (mockMedia.position + 1.2) % 200;
    window.__DSH_REMOTE__.clockNow = Date.now();
  }, 1000);
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
