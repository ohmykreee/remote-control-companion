(function () {
  'use strict';
  if (window.top !== window) return;

  var config = __DSH_COMPANION_CONFIG__;
  if (location.origin !== config.origin) return;

  // Report the page language to the companion. The remote WebView has Tauri
  // IPC available; observing the page's own `lang` attribute is more reliable
  // than polling it from the host-side WebView2 script context.
  try {
    var reportLanguage = function (language) {
      if (!language) return;
      var webview = window.chrome && window.chrome.webview;
      if (!webview || typeof webview.postMessage !== 'function') return;
      webview.postMessage(
        JSON.stringify({ type: 'companion-language', language: language }),
      );
    };
    var syncLanguage = function () {
      var raw = String(document.documentElement.getAttribute('lang') || '').toLowerCase();
      var language = raw.indexOf('zh') === 0 ? 'zh' : raw.indexOf('en') === 0 ? 'en' : '';
      if (language) reportLanguage(language);
    };
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', syncLanguage, { once: true });
    } else {
      syncLanguage();
    }
    try {
      if (document.documentElement) {
        new MutationObserver(syncLanguage).observe(document.documentElement, {
          attributes: true,
          attributeFilter: ['lang'],
        });
      }
    } catch (_) {
      // The interval below still catches language changes.
    }
    setInterval(syncLanguage, 1000);
  } catch (_) {
    // Language sync is best-effort; the page continues to work without it.
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
