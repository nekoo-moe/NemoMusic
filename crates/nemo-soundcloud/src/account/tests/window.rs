//! A sign-in window that is still open when the user pastes a token or signs out, and the hint
//! and recheck that go to the window. The window here never answers by itself.

use std::sync::atomic::AtomicBool;

use super::*;

/// A window that waits for ever. Dropping its call (the app stopped it) is recorded.
#[derive(Default)]
struct HeldWindow {
    started: AtomicBool,
    stopped: Arc<AtomicBool>,
    hint: AtomicBool,
    rechecks: AtomicUsize,
}

/// Sets the flag when the waiting call is dropped.
struct OnDrop(Arc<AtomicBool>);

impl Drop for OnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl TokenSource for HeldWindow {
    fn token(&self) -> BoxFuture<'_, Result<Secret<String>, SourceAccountError>> {
        self.started.store(true, Ordering::SeqCst);
        let guard = OnDrop(Arc::clone(&self.stopped));
        Box::pin(async move {
            let _guard = guard;
            std::future::pending().await
        })
    }

    fn hint(&self) -> bool {
        self.hint.load(Ordering::SeqCst)
    }

    fn recheck(&self) {
        self.rechecks.fetch_add(1, Ordering::SeqCst);
    }
}

/// Let the spawned sign-in call run up to its wait.
async fn settle(window: &HeldWindow) {
    for _ in 0..50 {
        if window.started.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the window was never started");
}

async fn stopped(window: &HeldWindow) -> bool {
    for _ in 0..50 {
        if window.stopped.load(Ordering::SeqCst) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

#[tokio::test]
async fn a_pasted_token_stops_the_open_window_and_signs_in() {
    let window = Arc::new(HeldWindow::default());
    let rig = rig(
        "paste-over-window",
        Arc::clone(&window) as Arc<dyn TokenSource>,
    );
    let waiting = rig.account.sign_in();
    settle(&window).await;
    assert_eq!(rig.account.state(), SourceAccountState::SigningIn);

    let info = rig
        .account
        .sign_in_with_token(Secret::new(TOKEN.to_owned()))
        .await
        .expect("signed in");
    assert_eq!(waiting.await, Err(SourceAccountError::Cancelled));
    assert!(stopped(&window).await, "the window call was not stopped");
    assert_eq!(rig.account.state(), SourceAccountState::SignedIn(info));
    assert_eq!(stored(&rig).as_deref(), Some(TOKEN));
}

#[tokio::test]
async fn a_refused_paste_over_an_open_window_goes_back_to_the_state_before() {
    let window = Arc::new(HeldWindow::default());
    let rig = rig(
        "refused-over-window",
        Arc::clone(&window) as Arc<dyn TokenSource>,
    );
    *rig.site.me.lock().expect("lock") = MeAnswer::Status(401);
    let waiting = rig.account.sign_in();
    settle(&window).await;

    let error = rig
        .account
        .sign_in_with_token(Secret::new(TOKEN.to_owned()))
        .await
        .expect_err("refused");
    assert_eq!(error, SourceAccountError::Rejected);
    assert_eq!(waiting.await, Err(SourceAccountError::Cancelled));
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
    assert_eq!(stored(&rig), None);
}

#[tokio::test]
async fn sign_out_stops_an_open_window() {
    let window = Arc::new(HeldWindow::default());
    let rig = rig(
        "signout-window",
        Arc::clone(&window) as Arc<dyn TokenSource>,
    );
    let waiting = rig.account.sign_in();
    settle(&window).await;

    rig.account.sign_out().await.expect("signed out");
    assert_eq!(waiting.await, Err(SourceAccountError::Cancelled));
    assert!(stopped(&window).await, "the window call was not stopped");
    assert_eq!(rig.account.state(), SourceAccountState::SignedOut);
}

#[tokio::test]
async fn a_window_that_already_finished_is_not_undone_by_a_later_paste() {
    let rig = rig("after-window", FakeWindow::gives(TOKEN));
    let first = rig.account.sign_in().await.expect("window");
    *rig.site.me.lock().expect("lock") = MeAnswer::Status(401);
    let error = rig
        .account
        .sign_in_with_token(Secret::new(OTHER_TOKEN.to_owned()))
        .await
        .expect_err("refused");
    assert_eq!(error, SourceAccountError::Rejected);
    assert_eq!(rig.account.state(), SourceAccountState::SignedIn(first));
}

#[tokio::test]
async fn the_hint_and_the_recheck_go_to_the_window() {
    let window = Arc::new(HeldWindow::default());
    let rig = rig("hint", Arc::clone(&window) as Arc<dyn TokenSource>);
    assert!(!rig.account.sign_in_hint());
    window.hint.store(true, Ordering::SeqCst);
    assert!(rig.account.sign_in_hint());
    rig.account.recheck_sign_in();
    rig.account.recheck_sign_in();
    assert_eq!(window.rechecks.load(Ordering::SeqCst), 2);
}
