(function () {
  'use strict';
  if (window.top !== window) return;

  var config = __DSH_COMPANION_CONFIG__;
  if (location.origin !== config.origin) return;

  // Report language and theme to the companion. Windows uses WebView2's
  // web-message channel; WebKitGTK exposes the equivalent through its script
  // message handler. The interval also covers handlers installed just after
  // the first page load.
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
      // The page works without host-side language/theme hints.
    }
  };

  try {
    var syncRemoteState = function () {
      var raw = String(document.documentElement.getAttribute('lang') || '').toLowerCase();
      var language = raw.indexOf('zh') === 0 ? 'zh' : raw.indexOf('en') === 0 ? 'en' : '';
      if (language) {
        sendToCompanion({ type: 'companion-language', language: language });
      }

      var body = document.body;
      var theme = body && body.dataset ? body.dataset.theme || '' : '';
      if (theme !== 'light' && theme !== 'dark') {
        var html = document.documentElement;
        theme = html.classList.contains('light')
          ? 'light'
          : html.classList.contains('dark')
            ? 'dark'
            : '';
      }
      if (!theme) {
        var meta = document.querySelector('meta[name="theme-color"]');
        var color = meta && meta.content ? meta.content.trim().toLowerCase() : '';
        if (color === '#eef2f8') theme = 'light';
        else if (color === '#151a23') theme = 'dark';
      }
      if (theme === 'light' || theme === 'dark') {
        sendToCompanion({ type: 'companion-theme', theme: theme });
      }
    };
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', syncRemoteState, { once: true });
    } else {
      syncRemoteState();
    }
    try {
      if (document.documentElement) {
        new MutationObserver(syncRemoteState).observe(document.documentElement, {
          attributes: true,
          subtree: true,
          attributeFilter: ['lang', 'class', 'data-theme'],
        });
      }
    } catch (_) {
      // The interval below still catches changes.
    }
    setInterval(syncRemoteState, 1000);
  } catch (_) {
    // Language/theme sync is best-effort; the page works without it.
  }

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

  __DSH_COMPANION_VR_API__
})();
