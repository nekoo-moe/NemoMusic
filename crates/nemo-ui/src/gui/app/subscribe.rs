//! Event streams of the app: keys, player events and the frame timer.

use super::message::Message;
use crate::keys::{Key, Mods};
use iced::{keyboard, stream};
use nemo_core::services::PlayerControl;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub(super) fn map_key(event: keyboard::Event) -> Option<Message> {
    use keyboard::key::Named;
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };
    let mapped = match key.as_ref() {
        keyboard::Key::Named(Named::Space) => Key::Space,
        keyboard::Key::Named(Named::ArrowLeft) => Key::Left,
        keyboard::Key::Named(Named::ArrowRight) => Key::Right,
        keyboard::Key::Named(Named::ArrowUp) => Key::Up,
        keyboard::Key::Named(Named::ArrowDown) => Key::Down,
        keyboard::Key::Named(Named::Escape) => Key::Escape,
        keyboard::Key::Named(Named::F3) => Key::F3,
        keyboard::Key::Named(Named::Tab) => Key::Tab,
        keyboard::Key::Character(c) => Key::Char(c.chars().next()?.to_ascii_lowercase()),
        _ => return None,
    };
    Some(Message::Key(
        mapped,
        Mods {
            ctrl: modifiers.control() || modifiers.command(),
            shift: modifiers.shift(),
        },
    ))
}

/// Subscription identity: one stream per player.
pub(super) struct PlayerKey(pub(super) Arc<dyn PlayerControl>);

impl Hash for PlayerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The address of the player object identifies it.
        std::ptr::addr_of!(*self.0).cast::<()>().hash(state);
    }
}

/// Poll the player's event receiver on a plain thread (the trait is non-blocking and
/// runtime-agnostic) and forward real events only, so an idle player causes no UI work.
pub(super) fn player_events(key: &PlayerKey) -> impl iced::futures::Stream<Item = Message> + use<> {
    let player = Arc::clone(&key.0);
    stream::channel(
        64,
        async move |mut tx: iced::futures::channel::mpsc::Sender<Message>| {
            let mut rx = player.subscribe();
            thread::spawn(move || {
                loop {
                    let mut sent = false;
                    while let Some(ev) = rx.try_next() {
                        if tx.try_send(Message::Player(ev)).is_err() && tx.is_closed() {
                            return;
                        }
                        sent = true;
                    }
                    if !sent && tx.is_closed() {
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
        },
    )
}

/// One tick per second while a sleep timer runs (a plain thread, like the player events).
pub(super) fn timer_ticks() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(
        4,
        async move |mut tx: iced::futures::channel::mpsc::Sender<Message>| {
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    if tx.try_send(Message::TimerTick).is_err() && tx.is_closed() {
                        return;
                    }
                }
            });
        },
    )
}
