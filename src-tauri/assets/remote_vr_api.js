  // Screenshot API consumed by dsh-remote-control. Kept in its own closure so
  // the page only sees a frozen object with one callable method.
  (function () {
    if (!config.vrScreenshot || !config.bridgeUrl) return;
    if (window.__DSH_COMPANION__) return;

    var socket = null;
    var connecting = null;
    var pending = new Map();
    var sequence = 0;
    var TIMEOUT_MS = 35000;

    function failAll(message) {
      pending.forEach(function (entry) {
        clearTimeout(entry.timer);
        entry.reject(new Error(message));
      });
      pending.clear();
    }

    function handleMessage(event) {
      if (typeof event.data === 'string') {
        var payload;
        try {
          payload = JSON.parse(event.data);
        } catch (_) {
          return;
        }
        if (!payload || !payload.id) return;
        var failed = pending.get(payload.id);
        if (!failed) return;
        pending.delete(payload.id);
        clearTimeout(failed.timer);
        failed.reject(new Error(payload.error || 'VR 截图失败，请确认 SteamVR 正在运行且头显已唤醒。'));
        return;
      }

      if (!(event.data instanceof ArrayBuffer) || event.data.byteLength < 4) return;
      var view = new DataView(event.data);
      var headerLength = view.getUint32(0, true);
      if (headerLength < 2 || 4 + headerLength > event.data.byteLength) return;
      var header;
      try {
        header = JSON.parse(new TextDecoder().decode(new Uint8Array(event.data, 4, headerLength)));
      } catch (_) {
        return;
      }
      if (!header || !header.id) return;
      var entry = pending.get(header.id);
      if (!entry) return;
      pending.delete(header.id);
      clearTimeout(entry.timer);
      var body = new Uint8Array(event.data, 4 + headerLength);
      entry.resolve(new Blob([body], { type: header.mime || 'image/png' }));
    }

    function connect() {
      if (socket && socket.readyState === WebSocket.OPEN) return Promise.resolve(socket);
      if (connecting) return connecting;
      connecting = new Promise(function (resolve, reject) {
        var ws;
        try {
          ws = new WebSocket(config.bridgeUrl);
        } catch (_) {
          connecting = null;
          reject(new Error('VR 截图服务没有启动，请重新打开本软件。'));
          return;
        }
        ws.binaryType = 'arraybuffer';
        var timer = setTimeout(function () {
          connecting = null;
          try { ws.close(); } catch (_) {}
          reject(new Error('VR 截图服务没有响应，请重新打开本软件。'));
        }, 8000);

        ws.onopen = function () {
          clearTimeout(timer);
          socket = ws;
          connecting = null;
          resolve(ws);
        };
        ws.onerror = function () {
          clearTimeout(timer);
          connecting = null;
          reject(new Error('VR 截图服务没有响应，请重新打开本软件。'));
        };
        ws.onclose = function () {
          clearTimeout(timer);
          if (socket === ws) socket = null;
          connecting = null;
          failAll('VR 截图服务连接已断开，请重新载入远控页面后重试。');
        };
        ws.onmessage = handleMessage;
      });
      return connecting;
    }

    function captureVrScreenshot() {
      return new Promise(function (resolve, reject) {
        connect().then(function (ws) {
          var id = String(++sequence);
          var timer = setTimeout(function () {
            var entry = pending.get(id);
            if (!entry) return;
            pending.delete(id);
            entry.reject(new Error('VR 截图超时，请确认 SteamVR 正在运行且头显已唤醒。'));
          }, TIMEOUT_MS);
          pending.set(id, { resolve: resolve, reject: reject, timer: timer });
          try {
            ws.send(JSON.stringify({ type: 'capture-vr-screenshot', id: id }));
          } catch (_) {
            clearTimeout(timer);
            pending.delete(id);
            reject(new Error('VR 截图请求没有发送成功，请重新载入远控页面后重试。'));
          }
        }).catch(reject);
      });
    }

    var api = Object.freeze({
      version: 1,
      platform: 'windows',
      capabilities: Object.freeze({ vrScreenshot: true }),
      captureVrScreenshot: captureVrScreenshot,
    });
    Object.defineProperty(window, '__DSH_COMPANION__', {
      value: api,
      writable: false,
      enumerable: true,
      configurable: false,
    });
    window.dispatchEvent(new CustomEvent('dsh-companion-ready', { detail: { version: 1 } }));
  })();
