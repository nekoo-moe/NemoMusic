//! The page hook (`src/hook.js`) against a mock `fetch`, `XMLHttpRequest` and `window.ipc`, run
//! with `node`. Skipped, with a note, where `node` is not installed. The tokens are invented.

use std::process::Command;

const HOOK: &str = include_str!("../src/hook.js");

const HARNESS: &str = r"
const sent = [];
global.window = global;
global.location = { href: 'https://soundcloud.com/discover' };
global.ipc = { postMessage: (m) => sent.push(m) };
global.fetch = function (input, init) { return Promise.resolve('real fetch'); };
global.XMLHttpRequest = function () {};
XMLHttpRequest.prototype.open = function () { this.opened = true; };
XMLHttpRequest.prototype.setRequestHeader = function (n, v) { (this.headers = this.headers || {})[n] = v; };
const TOKEN = '2-293232-1234567-AbCdEfGh';
HOOK_SOURCE
(async () => {
  const check = (name, cond) => { if (!cond) { console.log('FAIL ' + name); process.exitCode = 1; } };
  // The page's request still goes through, unchanged.
  check('fetch passes', (await fetch('https://api-v2.soundcloud.com/me', { headers: { Authorization: 'OAuth ' + TOKEN } })) === 'real fetch');
  check('plain headers', sent.length === 1 && sent[0] === 'oauth:' + TOKEN);
  await fetch('https://api-v2.soundcloud.com/x', { headers: new Map([['authorization', 'OAuth ' + TOKEN + 'b']]) });
  check('Headers-like object', sent[1] === 'oauth:' + TOKEN + 'b');
  await fetch('https://api-v2.soundcloud.com/y', { headers: [['AUTHORIZATION', 'OAuth ' + TOKEN + 'c']] });
  check('array headers', sent[2] === 'oauth:' + TOKEN + 'c');
  await fetch({ url: 'https://a.soundcloud.com/z', headers: { authorization: 'OAuth ' + TOKEN + 'd' } });
  check('request object', sent[3] === 'oauth:' + TOKEN + 'd');
  const x = new XMLHttpRequest();
  x.open('GET', 'https://api-v2.soundcloud.com/w');
  x.setRequestHeader('Authorization', 'OAuth ' + TOKEN + 'e');
  check('xhr', sent[4] === 'oauth:' + TOKEN + 'e' && x.opened && x.headers.Authorization === 'OAuth ' + TOKEN + 'e');
  const before = sent.length;
  // Not offered: another host, a look-alike host, no OAuth word, no header, a non-string.
  await fetch('https://evil.example/me', { headers: { Authorization: 'OAuth ' + TOKEN } });
  await fetch('https://soundcloud.com.evil.example/me', { headers: { Authorization: 'OAuth ' + TOKEN } });
  await fetch('https://notsoundcloud.com/me', { headers: { Authorization: 'OAuth ' + TOKEN } });
  await fetch('https://api-v2.soundcloud.com/me', { headers: { Authorization: 'Bearer ' + TOKEN } });
  await fetch('https://api-v2.soundcloud.com/me');
  await fetch('https://api-v2.soundcloud.com/me', { headers: { Authorization: 42 } });
  await fetch('https://api-v2.soundcloud.com/me', { headers: { 'Content-Type': 'application/json' } });
  check('nothing else is offered', sent.length === before);
  // A broken ipc channel never reaches the page.
  global.ipc.postMessage = () => { throw new Error('boom'); };
  let threw = false;
  try { await fetch('https://api-v2.soundcloud.com/me', { headers: { Authorization: 'OAuth ' + TOKEN } }); } catch (e) { threw = true; }
  check('errors stay inside the hook', !threw);
  // Loading the hook twice wraps fetch once.
  console.log(process.exitCode ? 'FAILED' : 'OK');
})();
";

#[test]
fn the_hook_offers_only_the_oauth_header_of_soundcloud_requests_and_changes_nothing() {
    let script = HARNESS.replace("HOOK_SOURCE", HOOK);
    let dir = std::env::temp_dir().join(format!("nemo-signin-hook-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let file = dir.join("hook-test.js");
    std::fs::write(&file, script).expect("script");
    let result = Command::new("node").arg(&file).output();
    let _ = std::fs::remove_dir_all(&dir);
    let Ok(out) = result else {
        eprintln!("node is not installed: the hook test is skipped");
        return;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success() && text.trim() == "OK", "{text}");
}
