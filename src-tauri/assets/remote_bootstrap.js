(function () {
  'use strict';
  if (window.top !== window) return;

  var config = __DSH_COMPANION_CONFIG__;
  if (location.origin !== config.origin) return;

  // Report the page's language, theme and live console data through the single
  // host message channel. Windows uses WebView2's web-message channel;
  // WebKitGTK exposes the equivalent through its script message handler.
  var sendToCompanion = function (message) {
    var payload = JSON.stringify(message);
    try {
      var webkit = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.dshCompanion;
      if (webkit && typeof webkit.postMessage === 'function') {
        webkit.postMessage(payload);
        return;
      }
    } catch (_) {
      // Fall through to the WebView2 channel.
    }
    try {
      var webview = window.chrome && window.chrome.webview;
      if (webview && typeof webview.postMessage === 'function') {
        webview.postMessage(payload);
      }
    } catch (_) {
      // The page works without host-side state hints.
    }
  };

  var readRemoteState = function () {
    var html = document.documentElement;
    var raw = String((html && html.getAttribute('lang')) || '').toLowerCase();
    var language = raw.indexOf('zh') === 0 ? 'zh' : raw.indexOf('en') === 0 ? 'en' : '';

    var body = document.body;
    var theme = body && body.dataset ? body.dataset.theme || '' : '';
    if (theme !== 'light' && theme !== 'dark') {
      theme = html && html.classList.contains('light')
        ? 'light'
        : html && html.classList.contains('dark')
          ? 'dark'
          : '';
    }
    if (!theme) {
      var meta = document.querySelector('meta[name="theme-color"]');
      var color = meta && meta.content ? meta.content.trim().toLowerCase() : '';
      if (color === '#eef2f8') theme = 'light';
      else if (color === '#151a23') theme = 'dark';
    }
    return { language: language, theme: theme };
  };

  var reduceMotionStyle = function (enabled) {
    var style = document.getElementById('dsh-reduce-motion-style');
    if (!enabled) {
      if (style) style.remove();
      document.documentElement.classList.remove('dsh-reduce-motion');
      return;
    }
    document.documentElement.classList.add('dsh-reduce-motion');
    if (style) return;
    style = document.createElement('style');
    style.id = 'dsh-reduce-motion-style';
    style.textContent =
      '*,*::before,*::after{animation-duration:0.001ms !important;' +
      'animation-iteration-count:1 !important;transition-duration:0.001ms !important;' +
      'scroll-behavior:auto !important;}';
    (document.head || document.documentElement).appendChild(style);
  };
  window.__DSH_SET_REDUCED_MOTION__ = reduceMotionStyle;
  try {
    reduceMotionStyle(!!config.reduceMotion);
  } catch (_) {
    // Reduced motion is an enhancement; ignore stylesheet failures.
  }

  // Use the microphone chosen in the companion settings. Device ids in a
  // browser are scoped to the page origin, so the stored label is matched
  // against the remote page's own device list.
  try {
    var media = navigator.mediaDevices;
    if (config.microphoneLabel && media && typeof media.getUserMedia === 'function') {
      var originalGetUserMedia = media.getUserMedia.bind(media);
      var warmedLabels = false;

      var findSelectedDevice = async function () {
        var devices = await media.enumerateDevices();
        var inputs = devices.filter(function (device) {
          return device.kind === 'audioinput' && !!device.label;
        });
        var match = inputs.find(function (device) {
          return device.label === config.microphoneLabel;
        });
        if (match) return match;
        if (!warmedLabels) {
          warmedLabels = true;
          try {
            var warm = await originalGetUserMedia({ audio: true });
            warm.getTracks().forEach(function (track) { track.stop(); });
          } catch (_) {
            // Keep the page's own request path as the fallback.
          }
          devices = await media.enumerateDevices();
          inputs = devices.filter(function (device) {
            return device.kind === 'audioinput' && !!device.label;
          });
          match = inputs.find(function (device) {
            return device.label === config.microphoneLabel;
          });
        }
        return match || null;
      };

      media.getUserMedia = async function (constraints) {
        if (!constraints || constraints.audio === undefined || constraints.audio === false) {
          return originalGetUserMedia(constraints);
        }
        try {
          var device = await findSelectedDevice();
          if (device) {
            var audio = typeof constraints.audio === 'object' && constraints.audio !== null
              ? Object.assign({}, constraints.audio)
              : {};
            audio.deviceId = { exact: device.deviceId };
            return originalGetUserMedia(Object.assign({}, constraints, { audio: audio }));
          }
        } catch (_) {
          // Fall through to the unmodified request.
        }
        return originalGetUserMedia(constraints);
      };
    }
  } catch (_) {
    // Browser versions that lock mediaDevices stay on the system default.
  }

  // Report the remote console's live data for the VRChat chatbox panel. Only
  // the fields the panel needs are copied out of the page's own state; the
  // companion formats and sends the OSC message.
  try {
    var mediaAnchor = { key: '', position: 0, at: 0 };

    var finiteNumber = function (value) {
      if (value === null || value === undefined || value === '') return null;
      var number = Number(value);
      return Number.isFinite(number) ? number : null;
    };
    var textValue = function (value) {
      return typeof value === 'string' ? value : value === null || value === undefined ? '' : String(value);
    };

    var clockParts = function () {
      var now = new Date();
      var pad = function (value) { return String(value).padStart(2, '0'); };
      var lang = String(document.documentElement.getAttribute('lang') || '').toLowerCase();
      var locale = lang.indexOf('en') === 0 ? 'en-US' : 'zh-CN';
      var weekday = '';
      try {
        weekday = new Intl.DateTimeFormat(locale, { weekday: 'short' }).format(now);
      } catch (_) {
        weekday = '';
      }
      return {
        date: pad(now.getMonth() + 1) + '-' + pad(now.getDate()),
        weekday: weekday,
        time: pad(now.getHours()) + ':' + pad(now.getMinutes())
      };
    };

    var mediaProgress = function (media, playing) {
      var raw = finiteNumber(media.position);
      var start = finiteNumber(media.startTime);
      var end = finiteNumber(media.endTime);
      var duration = 0;
      if (end !== null) {
        duration = Math.max(0, end - (start !== null ? start : 0));
        if (!(duration > 0)) duration = Math.max(0, end);
      }
      var position = raw !== null ? Math.max(0, raw) : 0;
      var key = [textValue(media.title), textValue(media.artist), start, end].join('\u0001');
      var now = Date.now();
      if (key !== mediaAnchor.key || raw === null) {
        mediaAnchor = { key: key, position: position, at: now };
      } else if (
        Math.abs(raw - mediaAnchor.position) > 0.75 ||
        raw < mediaAnchor.position - 2 ||
        raw > mediaAnchor.position + 4
      ) {
        mediaAnchor.position = raw;
        mediaAnchor.at = now;
      }
      if (playing) position = mediaAnchor.position + (now - mediaAnchor.at) / 1000;
      if (duration > 0) position = Math.min(position, duration);
      return { position: Math.max(0, position), duration: duration };
    };

    var weatherStatus = function (app) {
      var weather = app.weather;
      var loaded = weather !== null && weather !== undefined;
      if (!loaded || weather.ok === false) {
        return { loaded: loaded, available: false };
      }
      var current = weather.current || {};
      var daily = weather.daily || {};
      var unit = String(app.temperatureUnit || 'celsius').toLowerCase() === 'fahrenheit' ? 'F' : 'C';
      var convert = function (value) {
        var number = finiteNumber(value);
        if (number === null) return null;
        return Math.round(unit === 'F' ? (number * 9) / 5 + 32 : number);
      };
      return {
        loaded: true,
        available: true,
        location: weather.location && !weather.location.nameFallback ? textValue(weather.location.name) : '',
        code: finiteNumber(current.code),
        isDay: typeof current.isDay === 'boolean' ? current.isDay : null,
        temperature: convert(current.temperature),
        high: convert(daily.high),
        low: convert(daily.low),
        unit: unit
      };
    };

    var musicStatus = function (app) {
      var payload = app.media;
      var media = payload && payload.media && payload.media.media;
      if (!media) return { has: false };
      var playing = String(media.playbackStatus || '').toLowerCase() === 'playing';
      var progress = mediaProgress(media, playing);
      return {
        has: true,
        title: textValue(media.title),
        artist: textValue(media.artist || media.albumArtist),
        album: textValue(media.album),
        playing: playing,
        position: progress.position,
        duration: progress.duration
      };
    };

    var systemStatus = function (app) {
      var performance = app.performance;
      var cpu = (performance && performance.cpu) || {};
      var memory = (performance && performance.memory) || {};
      var gpu = (performance && performance.gpu) || {};
      return {
        cpuModel: textValue(cpu.model),
        cpuUsage: finiteNumber(cpu.usage),
        memUsed: finiteNumber(memory.used),
        memTotal: finiteNumber(memory.total),
        memPercent: finiteNumber(memory.percent),
        gpuOk: gpu.ok !== false,
        gpuName: textValue(gpu.name),
        gpuUsage: finiteNumber(gpu.utilization),
        vramUsed: finiteNumber(gpu.memoryUsed),
        vramTotal: finiteNumber(gpu.memoryTotal),
        vramPercent: finiteNumber(gpu.memoryPercent)
      };
    };

    var agentStatus = function (app) {
      var sessions = Array.isArray(app.sessions) ? app.sessions : [];
      var running = 0;
      var waiting = 0;
      var errors = 0;
      sessions.forEach(function (session) {
        var status = session && session.status;
        if (status === 'running') running += 1;
        else if (status === 'question') waiting += 1;
        else if (status === 'error') errors += 1;
      });
      var account = app.account;
      var loaded = account !== null && account !== undefined;
      var signedIn = loaded && account.available !== false && !!account.signedIn;
      var balance = (account && account.balance) || {};
      var usage = (account && account.usage) || {};
      return {
        running: running,
        waiting: waiting,
        errors: errors,
        accountLoaded: loaded,
        signedIn: signedIn,
        lowBalance: !!(account && account.lowBalance),
        balance: signedIn ? finiteNumber(balance.balance) : null,
        currency: textValue(balance.currency || usage.currency || 'CNY'),
        todaySpend: finiteNumber(usage.today)
      };
    };

    var collectStatus = function () {
      var app = window.__DSH_REMOTE__;
      if (!app || typeof app !== 'object') return null;
      var clock = clockParts();
      return {
        date: clock.date,
        weekday: clock.weekday,
        time: clock.time,
        weather: weatherStatus(app),
        music: musicStatus(app),
        system: systemStatus(app),
        agent: agentStatus(app)
      };
    };

    var reportStatus = function () {
      try {
        var status = collectStatus();
        if (status) sendToCompanion({ type: 'companion-status', status: status });
      } catch (_) {
        // Status is optional; the page keeps working without it.
      }
    };

    var lastLanguage = '';
    var lastTheme = '';
    var reportUiState = function () {
      try {
        var state = readRemoteState();
        var languageChanged = state.language && state.language !== lastLanguage;
        var themeChanged = state.theme && state.theme !== lastTheme;
        if (!languageChanged && !themeChanged) return;
        if (languageChanged) lastLanguage = state.language;
        if (themeChanged) lastTheme = state.theme;
        var message = { type: 'companion-ui' };
        if (languageChanged) message.language = state.language;
        if (themeChanged) message.theme = state.theme;
        sendToCompanion(message);
      } catch (_) {
        // UI hints are optional; the page keeps working without them.
      }
    };

    var uiObserverInstalled = false;
    var installUiObserver = function () {
      if (uiObserverInstalled) return;
      var root = document.documentElement;
      if (!root) {
        setTimeout(installUiObserver, 50);
        return;
      }
      try {
        new MutationObserver(reportUiState).observe(root, {
          attributes: true,
          subtree: true,
          attributeFilter: ['lang', 'class', 'data-theme'],
        });
        uiObserverInstalled = true;
        reportUiState();
      } catch (_) {
        setTimeout(installUiObserver, 500);
      }
    };
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', function () {
        installUiObserver();
        reportStatus();
      }, { once: true });
    } else {
      installUiObserver();
      reportStatus();
    }
    setInterval(function () {
      installUiObserver();
      reportStatus();
    }, 1000);
  } catch (_) {
    // Chatbox status is best-effort.
  }

  var installOscPush = function () {
    if (!config.oscPush) return;
    var button = null;
    var selectedText = '';

    var hide = function () {
      if (button) {
        button.remove();
        button = null;
      }
      selectedText = '';
    };

    var inAnswer = function (node) {
      if (!node) return false;
      var element = node.nodeType === 1 ? node : node.parentElement;
      return !!(element && element.closest && element.closest('.quickqa-chat-assistant'));
    };

    var show = function (rect) {
      if (!button) {
        button = document.createElement('button');
        button.type = 'button';
        button.className = 'dsh-osc-push';
        button.style.cssText =
          'position:fixed;z-index:2147483647;border:1px solid rgba(255,255,255,.45);border-radius:999px;' +
          'background:#2563eb;color:#ffffff;font-family:inherit;font-size:12px;font-weight:600;line-height:1.2;' +
          'padding:7px 11px;box-shadow:0 0 0 2px rgba(37,99,235,.35),0 8px 22px rgba(0,0,0,.45);' +
          'text-shadow:0 1px 1px rgba(0,0,0,.22);cursor:pointer;user-select:none;-webkit-user-select:none;';
        button.addEventListener('click', function (event) {
          event.preventDefault();
          event.stopPropagation();
          if (selectedText) sendToCompanion({ type: 'osc-push', text: selectedText });
          try {
            window.getSelection().removeAllRanges();
          } catch (_) {}
          hide();
        });
        document.body.appendChild(button);
      }
      var english =
        String(document.documentElement.lang || '')
          .toLowerCase()
          .indexOf('en') === 0;
      button.textContent = english ? 'Push to OSC' : '推送到 OSC';
      var left = Math.min(Math.max(8, rect.left + rect.width / 2 - 44), window.innerWidth - 96);
      var top = rect.bottom + 8;
      if (top + 32 > window.innerHeight) top = Math.max(8, rect.top - 40);
      button.style.left = left + 'px';
      button.style.top = top + 'px';
    };

    var checkSelection = function () {
      try {
        var selection = window.getSelection();
        if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return hide();
        var range = selection.getRangeAt(0);
        if (!inAnswer(range.startContainer) || !inAnswer(range.endContainer)) return hide();
        // Selection.toString() is the rendered answer text, not the Markdown
        // source. Collapse every copied newline/whitespace run so the pushed
        // content is one plain paragraph.
        var text = selection.toString().replace(/\s+/g, ' ').trim();
        if (text.length < 2) return hide();
        selectedText = text.slice(0, 2000);
        var rect = range.getBoundingClientRect();
        if (!rect || (!rect.width && !rect.height)) {
          var rects = range.getClientRects();
          if (!rects.length) return hide();
          rect = rects[rects.length - 1];
        }
        show(rect);
      } catch (_) {
        hide();
      }
    };

    document.addEventListener(
      'mouseup',
      function () {
        setTimeout(checkSelection, 0);
      },
      true
    );
    document.addEventListener(
      'keyup',
      function () {
        setTimeout(checkSelection, 0);
      },
      true
    );
    document.addEventListener(
      'mousedown',
      function (event) {
        if (button && event.target !== button) hide();
      },
      true
    );
    document.addEventListener('scroll', hide, true);
    window.addEventListener('resize', hide);
    document.addEventListener('selectionchange', function () {
      var selection = window.getSelection();
      if (!selection || selection.isCollapsed) hide();
    });
  };
  try {
    installOscPush();
  } catch (_) {
    // The selection button is optional; the page keeps working without it.
  }

})();
