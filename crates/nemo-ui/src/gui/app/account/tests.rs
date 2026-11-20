//! The accounts sheet as the app runs it (source-workspaces spec): it opens from every state of
//! both accounts, forgets a half-typed SoundCloud token when it closes, and sends the person to
//! Settings from the SoundCloud card.

use nemo_core::secret::Secret;
use nemo_core::source_account::SourceAccountState;

use super::super::{App, Message, ScMsg, SettingsSection};
use crate::gui::headless::soundcloud::ScScene;
use crate::gui::headless::{Scenario, build_app};
use crate::mock::{MockScAccount, mock_backend, mock_backend_soundcloud};
use crate::route::Route;

fn with_soundcloud(state: SourceAccountState) -> App {
    build_app(
        &mock_backend_soundcloud(state),
        &Scenario {
            soundcloud: Some(ScScene::Account),
            ..Scenario::default()
        },
    )
}

fn open(app: &mut App) {
    let _ = app.update_core(Message::Account(AccountMsg::Open));
}

use super::AccountMsg;

#[test]
fn the_sheet_opens_while_youtube_music_is_signed_out_and_in_every_soundcloud_state() {
    // The mock YouTube Music account starts signed out.
    for state in [
        SourceAccountState::SignedOut,
        SourceAccountState::SignedIn(MockScAccount::info()),
        SourceAccountState::Expired,
    ] {
        let mut app = with_soundcloud(state);
        assert!(!app.signed_in(), "YouTube Music is signed out");
        open(&mut app);
        assert!(app.account.open);
    }
    let mut plain = build_app(&mock_backend(), &Scenario::default());
    open(&mut plain);
    assert!(plain.account.open, "and with no SoundCloud at all");
}

#[test]
fn closing_the_sheet_forgets_a_typed_token_a_question_and_a_result_line() {
    let mut app = with_soundcloud(SourceAccountState::SignedOut);
    open(&mut app);
    let _ = app.update_core(Message::Sc(ScMsg::PasteMode(true)));
    let _ = app.update_core(Message::Sc(ScMsg::PasteInput(Secret::new(
        "typed-but-never-sent".to_owned(),
    ))));
    app.sc.notice = Some("Signed out.".to_owned());
    assert!(app.sc.paste && !app.sc.token.expose().is_empty());
    let _ = app.update_core(Message::Account(AccountMsg::Close));
    assert!(!app.account.open);
    assert!(!app.sc.paste && app.sc.token.expose().is_empty());
    assert!(app.sc.notice.is_none() && !app.sc.confirm_sign_out);
    assert!(!app.ui.text_focused, "the keys work again");
}

#[test]
fn the_turn_on_button_closes_the_sheet_and_opens_the_soundcloud_section() {
    let mut app = build_app(&mock_backend(), &Scenario::default());
    open(&mut app);
    let _ = app.update_core(Message::SettingsSectionGo(SettingsSection::SoundCloud));
    assert!(!app.account.open);
    assert_eq!(app.settings_section, SettingsSection::SoundCloud);
    assert_eq!(app.ui.route(), &Route::Settings);
}

#[test]
fn a_sign_out_confirmation_in_the_sheet_is_the_one_of_the_settings_card() {
    // One state, two places: the sheet and Settings read `App::sc`.
    let mut app = with_soundcloud(SourceAccountState::SignedIn(MockScAccount::info()));
    open(&mut app);
    let _ = app.update_core(Message::Sc(ScMsg::AskSignOut));
    assert!(app.sc.confirm_sign_out);
    let _ = app.update_core(Message::Sc(ScMsg::CancelSignOut));
    assert!(!app.sc.confirm_sign_out);
}
