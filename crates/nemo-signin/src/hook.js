// Watches the page's own requests to soundcloud.com hosts for `Authorization: OAuth <token>`,
// which the web app sends once the user is signed in, and tells the sign-in window the token
// through its IPC channel. It reads nothing else, changes no request and answers no page.
(function () {
  if (window.__nemoSigninHook) { return; }
  window.__nemoSigninHook = true;
  function offer(url, value) {
    try {
      if (typeof value !== 'string' || value.slice(0, 6) !== 'OAuth ') { return; }
      var host = new URL(url, location.href).hostname;
      if (host !== 'soundcloud.com' && !host.endsWith('.soundcloud.com')) { return; }
      window.ipc.postMessage('oauth:' + value.slice(6));
    } catch (e) { /* the page must never see an error from this hook */ }
  }
  function header(headers, name) {
    if (!headers) { return null; }
    if (typeof headers.get === 'function') { return headers.get(name); }
    if (Array.isArray(headers)) {
      for (var i = 0; i < headers.length; i++) {
        if (String(headers[i][0]).toLowerCase() === name) { return headers[i][1]; }
      }
      return null;
    }
    for (var key in headers) {
      if (String(key).toLowerCase() === name) { return headers[key]; }
    }
    return null;
  }
  var realFetch = window.fetch;
  if (typeof realFetch === 'function') {
    window.fetch = function (input, init) {
      try {
        var url = typeof input === 'string' ? input : (input && input.url) || '';
        offer(url, header(init && init.headers, 'authorization') || header(input && input.headers, 'authorization'));
      } catch (e) { /* ignored */ }
      return realFetch.apply(this, arguments);
    };
  }
  var proto = window.XMLHttpRequest && window.XMLHttpRequest.prototype;
  if (proto) {
    var realOpen = proto.open;
    var realSet = proto.setRequestHeader;
    proto.open = function (method, url) {
      try { this.__nemoUrl = url; } catch (e) { /* ignored */ }
      return realOpen.apply(this, arguments);
    };
    proto.setRequestHeader = function (name, value) {
      try {
        if (String(name).toLowerCase() === 'authorization') { offer(this.__nemoUrl || '', value); }
      } catch (e) { /* ignored */ }
      return realSet.apply(this, arguments);
    };
  }
})();
