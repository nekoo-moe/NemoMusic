//! `nemo-signin`: shows a sign-in page in a webview window (`WebView2` on Windows, `WebKitGTK`
//! on Linux) and hands the session back through the stdout pipe. Two sites: Google's page for
//! YouTube Music (the default) and SoundCloud's own page (`--site soundcloud`).
//!
//! This is a library linked into `nemo-app`. The window still runs in its own process, so its
//! tao/wry event loop never shares a process with iced: `nemo-ytm` or `nemo-soundcloud` starts
//! the app executable again as `nemo-app --signin [--site soundcloud] --data-dir <folder>`, and
//! the app's `main` hands everything after [`SIGNIN_ARG`] to [`run`] before anything else
//! starts. The folder is this window's own webview profile; the account deletes it on sign-out.
//!
//! Exit codes: 0 signed in (one JSON line on stdout: `{"cookie":"a=b; c=d"}` for Google, and
//! `{"token":"…"}` for SoundCloud, which is only the value of its `oauth_token` cookie), 2 the
//! user closed the window (nothing printed), 3 the webview runtime or the window is not
//! available, 64 bad arguments. Cookies and tokens are never written to stderr, logs or files.

mod cookies;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::{PageLoadEvent, WebContext, WebView, WebViewBuilder};

/// The first program argument that turns the app executable into the sign-in window.
pub const SIGNIN_ARG: &str = "--signin";
/// Exit code: the user closed the window.
pub const EXIT_CANCELLED: i32 = 2;
/// Exit code: the webview runtime or the window is not available.
pub const EXIT_UNAVAILABLE: i32 = 3;
/// Exit code: bad arguments.
pub const EXIT_USAGE: i32 = 64;

/// How often the SoundCloud window looks for the session cookie. The sign-in there can finish
/// without loading a new page, so a page-load event alone could miss it.
const SOUNDCLOUD_POLL: Duration = Duration::from_secs(1);

/// Sent from the page-load callback to the event loop.
enum Message {
    /// A page finished loading.
    Loaded(String),
}

/// Which sign-in page the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Site {
    /// Google's page for YouTube Music.
    Google,
    /// SoundCloud's own page.
    SoundCloud,
}

impl Site {
    /// The site named after `--site`.
    fn named(name: &std::ffi::OsStr) -> Option<Self> {
        match name.to_str()? {
            "youtube" | "google" => Some(Self::Google),
            "soundcloud" => Some(Self::SoundCloud),
            _ => None,
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Google => "Sign in to YouTube Music",
            Self::SoundCloud => "Sign in to SoundCloud",
        }
    }

    const fn url(self) -> &'static str {
        match self {
            Self::Google => cookies::SIGN_IN_URL,
            Self::SoundCloud => cookies::SOUNDCLOUD_SIGN_IN_URL,
        }
    }
}

/// The site and the folder, if the arguments are `[--site <name>] --data-dir <folder>`. Without
/// `--site` the window is Google's, as before.
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Option<(Site, PathBuf)> {
    let mut args = args.into_iter();
    let mut flag = args.next()?;
    let mut site = Site::Google;
    if flag == "--site" {
        site = Site::named(&args.next()?)?;
        flag = args.next()?;
    }
    let value = args.next()?;
    (flag == "--data-dir" && !value.is_empty() && args.next().is_none())
        .then(|| (site, PathBuf::from(value)))
}

/// Run the sign-in window with the arguments that follow [`SIGNIN_ARG`]
/// (`[--site soundcloud] --data-dir <folder>`).
///
/// Call it on the main thread, before any other GUI or async runtime starts in this process.
/// It returns the exit code when the window cannot run ([`EXIT_USAGE`], [`EXIT_UNAVAILABLE`]).
/// Once the window is open it never returns: it ends the process with 0 after the cookie line
/// is written, or with [`EXIT_CANCELLED`] when the user closes the window.
#[must_use]
pub fn run(args: impl IntoIterator<Item = OsString>) -> i32 {
    let Some((site, data_dir)) = parse_args(args) else {
        return EXIT_USAGE;
    };
    if std::fs::create_dir_all(&data_dir).is_err() {
        return EXIT_UNAVAILABLE;
    }
    let Some(event_loop) = build_event_loop() else {
        return EXIT_UNAVAILABLE;
    };
    show(event_loop, site, data_dir)
}

/// Build the event loop. tao panics when the windowing system cannot start (on Linux: no
/// display, GTK cannot initialise), so the panic is caught and reported as "not available".
fn build_event_loop() -> Option<EventLoop<Message>> {
    // The default hook would print the panic to stderr; nemo-ytm discards stderr anyway, and a
    // missing display is an expected outcome here, not a bug.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let built = std::panic::catch_unwind(|| EventLoopBuilder::<Message>::with_user_event().build());
    std::panic::set_hook(hook);
    built.ok()
}

/// Put the webview into the window. Windows: the native window handle (`WebView2`).
#[cfg(not(target_os = "linux"))]
fn attach(builder: WebViewBuilder<'_>, window: &tao::window::Window) -> wry::Result<WebView> {
    builder.build(window)
}

/// Put the webview into the window. Linux: tao's default GTK box (`WebKitGTK`), which works on
/// both X11 and Wayland.
#[cfg(target_os = "linux")]
fn attach(builder: WebViewBuilder<'_>, window: &tao::window::Window) -> wry::Result<WebView> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    let vbox = window
        .default_vbox()
        .ok_or(wry::Error::UnsupportedWindowHandle)?;
    builder.build_gtk(vbox)
}

/// Open the window and run its loop. Returns only when the window cannot be built; on success
/// the cookie line is already written and the process has ended.
fn show(event_loop: EventLoop<Message>, site: Site, data_dir: PathBuf) -> i32 {
    let proxy = event_loop.create_proxy();
    let Ok(window) = WindowBuilder::new()
        .with_title(site.title())
        .with_inner_size(LogicalSize::new(480.0, 720.0))
        .build(&event_loop)
    else {
        return EXIT_UNAVAILABLE;
    };

    let mut context = WebContext::new(Some(data_dir));
    // The user agent stays the webview's own: the sign-in pages check it, and a spoofed one is
    // what gets embedded sign-in blocked.
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(site.url())
        .with_navigation_handler(|url| cookies::is_allowed_navigation(&url))
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) {
                // The loop owns the webview; cookies are read there, on the UI thread.
                // Ignored on purpose: a send only fails when the loop has already ended.
                let _ = proxy.send_event(Message::Loaded(url));
            }
        });
    let Ok(webview) = attach(builder, &window) else {
        return EXIT_UNAVAILABLE;
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = match site {
            Site::Google => ControlFlow::Wait,
            Site::SoundCloud => ControlFlow::WaitUntil(Instant::now() + SOUNDCLOUD_POLL),
        };
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => std::process::exit(EXIT_CANCELLED),
            Event::UserEvent(Message::Loaded(url)) => {
                if let Some(line) = signed_in_line(site, &webview, &url) {
                    finish(&line);
                }
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) if site == Site::SoundCloud => {
                let on_site = webview.url().ok();
                if let Some(line) = on_site.and_then(|url| signed_in_line(site, &webview, &url)) {
                    finish(&line);
                }
            }
            _ => {}
        }
    })
}

/// The stdout line for the session of `site`, once the page `url` shows a signed-in session.
fn signed_in_line(site: Site, webview: &WebView, url: &str) -> Option<String> {
    match site {
        Site::Google => {
            if !cookies::is_music_page(url) {
                return None;
            }
            let jar = webview.cookies_for_url(cookies::MUSIC_ORIGIN).ok()?;
            let pairs = jar.iter().map(|c| (c.name(), c.value()));
            let header = cookies::cookie_header(pairs)?;
            Some(serde_json::json!({ "cookie": header }).to_string())
        }
        Site::SoundCloud => {
            if !cookies::is_soundcloud_page(url) {
                return None;
            }
            let jar = webview.cookies_for_url(cookies::SOUNDCLOUD_ORIGIN).ok()?;
            let all = jar.iter().map(|c| (c.name(), c.value(), c.domain()));
            cookies::soundcloud_token(all).map(|token| cookies::token_line(&token))
        }
    }
}

/// Write the line and end the process: 0 when it was written, [`EXIT_UNAVAILABLE`] when not.
fn finish(line: &str) -> ! {
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{line}").and_then(|()| out.flush());
    std::process::exit(if written.is_ok() { 0 } else { EXIT_UNAVAILABLE });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn data_dir_flag_is_required() {
        assert_eq!(
            parse_args(args(&["--data-dir", "C:/x"])),
            Some((Site::Google, PathBuf::from("C:/x")))
        );
        assert_eq!(parse_args(args(&[])), None);
        assert_eq!(parse_args(args(&["--data-dir"])), None);
        assert_eq!(parse_args(args(&["--data-dir", ""])), None);
        assert_eq!(parse_args(args(&["--other", "C:/x"])), None);
        assert_eq!(parse_args(args(&["--data-dir", "C:/x", "extra"])), None);
    }

    #[test]
    fn the_site_flag_picks_the_window() {
        assert_eq!(
            parse_args(args(&["--site", "soundcloud", "--data-dir", "C:/x"])),
            Some((Site::SoundCloud, PathBuf::from("C:/x")))
        );
        assert_eq!(
            parse_args(args(&["--site", "youtube", "--data-dir", "C:/x"])),
            Some((Site::Google, PathBuf::from("C:/x")))
        );
        for bad in [
            &["--site", "spotify", "--data-dir", "C:/x"][..],
            &["--site", "soundcloud"],
            &["--site", "soundcloud", "--data-dir"],
            &["--site", "soundcloud", "--data-dir", "C:/x", "extra"],
            &["--data-dir", "C:/x", "--site", "soundcloud"],
            &["--site", "--data-dir", "C:/x"],
        ] {
            assert_eq!(parse_args(args(bad)), None, "{bad:?}");
        }
    }

    #[test]
    fn each_site_has_its_own_title_and_https_address() {
        assert_eq!(Site::SoundCloud.url(), "https://soundcloud.com/signin");
        assert!(
            Site::Google
                .url()
                .starts_with("https://accounts.google.com/")
        );
        assert_ne!(Site::Google.title(), Site::SoundCloud.title());
    }

    #[test]
    fn bad_arguments_exit_with_the_usage_code() {
        assert_eq!(run(args(&[])), EXIT_USAGE);
        assert_eq!(run(args(&["--site", "spotify"])), EXIT_USAGE);
    }
}
