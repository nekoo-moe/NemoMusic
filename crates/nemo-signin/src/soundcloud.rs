//! The SoundCloud window's waiting for the session token.
//!
//! SoundCloud's sign-in can end without loading a page, so the window looks for the token on a
//! clock. The clock is a fixed deadline kept here: the event loop is woken every few milliseconds
//! by a busy page, and a deadline re-armed on each wake-up would never be reached.
//!
//! The token is read two ways and the first answer wins: the webview's cookie manager (every
//! cookie, `HttpOnly` ones too) and `document.cookie` evaluated in the page (what the web app's
//! own script sees). Only a cookie named `oauth_token` on a `soundcloud.com` host is ever kept;
//! every other cookie is dropped where it is read.
//!
//! With `NEMO_SIGNIN_DEBUG=1` the window writes one line per cookie to its stderr: name, domain,
//! flags, length and shape, never a value, plus the names found in `localStorage`,
//! `sessionStorage` and `window.__sc_hydration`.

use std::time::{Duration, Instant};

use serde_json::Value;
use wry::WebView;

use crate::cookies;
use crate::token::{self, document_cookie_lines, storage_line, token_from_document_cookie};

/// How often the window looks for the token.
pub(crate) const POLL: Duration = Duration::from_secs(1);
/// How long after a SoundCloud page loaded without a token the window tells the app (once), so
/// the app can offer "I am signed in, check again" and the token paste.
pub(crate) const HINT_AFTER: Duration = Duration::from_secs(25);
/// How often the diagnostic lists the cookies and the storage names.
const DEBUG_EVERY: Duration = Duration::from_secs(10);
/// The environment variable that turns the diagnostic on.
pub(crate) const DEBUG_ENV: &str = "NEMO_SIGNIN_DEBUG";

/// What the window reads from the page: `document.cookie`, as a JSON string.
const DOCUMENT_COOKIE_JS: &str = "(function(){try{return document.cookie}catch(e){return ''}})()";

/// What the diagnostic reads from the page: the names and lengths of the storage entries and the
/// names of the hydration entries, as one JSON text. No value is read into it.
const STORAGE_JS: &str = r"(function(){try{
var out={ls:[],ss:[],hy:[]};
function names(area,list){try{for(var i=0;i<area.length;i++){var k=area.key(i);
list.push([String(k).slice(0,80),String(area.getItem(k)||'').length]);}}catch(e){}}
names(window.localStorage,out.ls);names(window.sessionStorage,out.ss);
try{var h=window.__sc_hydration;if(Array.isArray(h)){for(var j=0;j<h.length;j++){
var x=h[j]||{};out.hy.push([String(x.hydratable).slice(0,40),JSON.stringify(x.data||'').length]);}}}catch(e){}
return JSON.stringify(out);}catch(e){return '{}'}})()";

/// The window's clock and what it has found so far.
#[derive(Debug)]
pub(crate) struct Poller {
    next: Instant,
    loaded_at: Option<Instant>,
    hint_sent: bool,
    next_debug: Instant,
    /// The next `document.cookie` answer is to be listed by the diagnostic.
    document_log: bool,
    /// The diagnostic is on.
    pub(crate) debug: bool,
}

impl Poller {
    /// A poller whose first look is at `now`.
    pub(crate) fn new(now: Instant, debug: bool) -> Self {
        Self {
            next: now,
            loaded_at: None,
            hint_sent: false,
            next_debug: now,
            document_log: false,
            debug,
        }
    }

    /// When the loop must wake up next.
    pub(crate) fn deadline(&self) -> Instant {
        self.next
    }

    /// `true` when a look is due at `now`; the next one is then [`POLL`] later, whatever else
    /// woke the loop in between.
    pub(crate) fn due(&mut self, now: Instant) -> bool {
        if now < self.next {
            return false;
        }
        self.next = now + POLL;
        true
    }

    /// Look at once (the app asked to check again).
    pub(crate) fn look_now(&mut self, now: Instant) {
        self.next = now;
    }

    /// A page finished loading. A page of `soundcloud.com` starts the wait for the hint.
    pub(crate) fn page_loaded(&mut self, url: &str, now: Instant) {
        if cookies::is_soundcloud_page(url) {
            self.loaded_at = Some(now);
        }
    }

    /// `true` once, when [`HINT_AFTER`] passed since the last SoundCloud page loaded and no
    /// token was found.
    pub(crate) fn hint_due(&mut self, now: Instant) -> bool {
        let due = !self.hint_sent
            && self
                .loaded_at
                .is_some_and(|at| now.saturating_duration_since(at) >= HINT_AFTER);
        if due {
            self.hint_sent = true;
        }
        due
    }

    /// The next `document.cookie` answer is the one the diagnostic asked for.
    pub(crate) fn expect_document_log(&mut self) {
        self.document_log = true;
    }

    /// Whether the answer that just came is to be listed (once per request).
    pub(crate) fn take_document_log(&mut self) -> bool {
        std::mem::take(&mut self.document_log)
    }

    /// `true` when the diagnostic should list the cookies now (its first look, then every
    /// [`DEBUG_EVERY`]).
    pub(crate) fn debug_due(&mut self, now: Instant) -> bool {
        if !self.debug || now < self.next_debug {
            return false;
        }
        self.next_debug = now + DEBUG_EVERY;
        true
    }
}

/// Write one diagnostic line to stderr. The lines hold names, flags and lengths only.
pub(crate) fn debug_line(line: &str) {
    eprintln!("signin {line}");
}

/// The token in the webview's cookie manager: the cookies for the site first, then every cookie
/// (the token can sit on another `soundcloud.com` host). With `debug`, one line per cookie.
pub(crate) fn token_from_cookie_manager(webview: &WebView, debug: bool) -> Option<String> {
    let for_site = webview.cookies_for_url(cookies::SOUNDCLOUD_ORIGIN).ok();
    let all = webview.cookies().ok();
    if debug {
        log_cookies("manager-site", for_site.as_deref().unwrap_or_default());
        log_cookies("manager-all", all.as_deref().unwrap_or_default());
        if for_site.is_none() || all.is_none() {
            debug_line("cookie manager answered with an error");
        }
    }
    for jar in [for_site, all].into_iter().flatten() {
        let all = jar.iter().map(|c| (c.name(), c.value(), c.domain()));
        if let Some(found) = cookies::soundcloud_token(all) {
            return Some(found);
        }
    }
    None
}

fn log_cookies(source: &str, jar: &[wry::cookie::Cookie<'static>]) {
    debug_line(&format!("{source}: {} cookies", jar.len()));
    for c in jar {
        debug_line(&token::cookie_line(
            source,
            c.name(),
            c.domain(),
            (c.secure(), c.http_only()),
            c.value(),
        ));
    }
}

/// Ask the page for `document.cookie`; the answer comes back through `deliver` as the JSON text
/// of the string.
pub(crate) fn ask_document_cookie(
    webview: &WebView,
    deliver: impl Fn(String) + Send + 'static,
) -> bool {
    webview
        .evaluate_script_with_callback(DOCUMENT_COOKIE_JS, deliver)
        .is_ok()
}

/// Ask the page for the diagnostic of its storage; the answer comes back through `deliver`.
pub(crate) fn ask_storage(webview: &WebView, deliver: impl Fn(String) + Send + 'static) {
    // A failed script only means the diagnostic has less to say.
    let _ = webview.evaluate_script_with_callback(STORAGE_JS, deliver);
}

/// The text of a JSON string answer (what the webview gives back for a script that returns a
/// string), or `None` for anything else.
pub(crate) fn json_string(answer: &str) -> Option<String> {
    serde_json::from_str::<String>(answer).ok()
}

/// What `document.cookie` said: the token, and with `debug` the lines about its cookies.
pub(crate) fn read_document_answer(answer: &str, debug: bool) -> Option<String> {
    let text = json_string(answer)?;
    if debug {
        let lines = document_cookie_lines(&text);
        debug_line(&format!("document.cookie: {} cookies", lines.len()));
        for line in lines {
            debug_line(&line);
        }
    }
    token_from_document_cookie(&text)
}

/// The diagnostic lines of a storage answer: names and lengths.
pub(crate) fn storage_lines(answer: &str) -> Vec<String> {
    let Some(text) = json_string(answer) else {
        return vec!["storage probe gave no answer".to_owned()];
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return vec!["storage probe gave no readable answer".to_owned()];
    };
    let mut out = Vec::new();
    for (key, area) in [
        ("ls", "localStorage"),
        ("ss", "sessionStorage"),
        ("hy", "hydration"),
    ] {
        let entries = value.get(key).and_then(Value::as_array);
        out.push(format!("{area}: {} entries", entries.map_or(0, Vec::len)));
        for entry in entries.into_iter().flatten().take(60) {
            let name = entry.get(0).and_then(Value::as_str).unwrap_or_default();
            let length = entry
                .get(1)
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(0);
            out.push(storage_line(area, name, length));
        }
    }
    out
}

#[cfg(test)]
mod tests;
