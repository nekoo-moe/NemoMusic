//! Read-only accessors of [`App`].

use super::super::style;
use super::App;
use super::art::{ArtEntry, ArtShape, ArtSpec};
use super::model::{DetailData, DetailKey, DetailKind, DetailState, ListSource};
use super::option::AccountLibrary;
use crate::collection_sort::{CollectionSort, page_key, sorted};
use crate::layout;
use crate::palette::Palette;
use crate::route::Route;
use iced::widget::image;
use nemo_core::account::AccountState;
use nemo_core::catalog::ShelfItem;
use nemo_core::track::{Artwork, Track};
use std::sync::Arc;

/// Title of the Home shelf the signed-in catalog fills with the account's playlists.
const YOUR_PLAYLISTS: &str = "Your playlists";
/// Title of the Home shelf the signed-in catalog fills with the account's liked songs.
const LIKED_SONGS: &str = "Liked songs";

impl App {
    /// The settings key of an album or playlist page; `None` for an artist page (no Sort).
    #[must_use]
    pub fn sort_key(key: &DetailKey) -> Option<String> {
        match key.kind {
            DetailKind::Album => Some(page_key(true, &key.id.0)),
            DetailKind::Playlist => Some(page_key(false, &key.id.0)),
            DetailKind::Artist => None,
        }
    }

    /// The order an album or playlist page shows its songs in (the remembered choice).
    #[must_use]
    pub fn collection_sort(&self, key: &DetailKey) -> CollectionSort {
        Self::sort_key(key)
            .and_then(|k| self.ui.settings.collection_sorts.get(&k).copied())
            .unwrap_or_default()
    }

    /// `true` when the parent wired playback speed (otherwise the chips are disabled).
    #[must_use]
    pub fn speed_wired(&self) -> bool {
        self.hooks.on_set_speed.is_some()
    }

    /// `true` when the app gave a callback for adding a music folder.
    #[must_use]
    pub fn has_add_folder(&self) -> bool {
        self.on_add_music_folder.is_some()
    }

    /// Window title.
    #[must_use]
    pub fn title(&self) -> String {
        match &self.ui.snapshot.current {
            Some(t) if self.ui.is_playing() => format!("{} \u{b7} NemoMusic", t.title),
            _ => "NemoMusic".to_owned(),
        }
    }

    /// The iced theme.
    #[must_use]
    pub fn theme(&self) -> iced::Theme {
        style::theme()
    }

    /// iced `style`: the window colour, transparent while the window glass is clear.
    #[must_use]
    pub fn style(&self, theme: &iced::Theme) -> iced::theme::Style {
        let base = iced::theme::Base::base(theme);
        if self.window_is_clear() {
            iced::theme::Style {
                background_color: iced::Color::TRANSPARENT,
                ..base
            }
        } else {
            base
        }
    }

    /// The accent palette to draw now (mid cross-fade it is a mix).
    #[must_use]
    pub fn accent(&self) -> Palette {
        self.ui.accent.current()
    }

    /// Current window size in logical px (rounded).
    #[must_use]
    pub fn window_px(&self) -> (u32, u32) {
        // Window sizes are small positive numbers.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (
            self.window.width.max(0.0) as u32,
            self.window.height.max(0.0) as u32,
        )
    }

    /// Picture for a slot. Returns `None` while it loads (the caller draws the placeholder) and
    /// records the request, which the app starts after the next update.
    pub fn art_handle(
        &self,
        artwork: &Artwork,
        size: u32,
        shape: ArtShape,
    ) -> Option<image::Handle> {
        self.art_shown(artwork, size, shape).map(|(h, _)| h)
    }

    /// [`App::art_handle`] with the opacity to draw it at: below 1 while it fades in.
    pub fn art_shown(
        &self,
        artwork: &Artwork,
        size: u32,
        shape: ArtShape,
    ) -> Option<(image::Handle, f32)> {
        let spec = ArtSpec {
            artwork: artwork.clone(),
            size,
            scale_pct: self.scale_pct,
            shape,
        };
        let key = spec.key()?;
        match self.art.get(&key) {
            Some(ArtEntry::Ready(h)) => Some((h.clone(), self.micro.art.opacity(&key))),
            Some(ArtEntry::Loading | ArtEntry::Missing) => None,
            None => {
                self.wants.borrow_mut().push(spec);
                None
            }
        }
    }

    /// `true` when nobody needs to sign in to like songs.
    #[must_use]
    pub fn signed_in(&self) -> bool {
        matches!(self.backend.account.state(), AccountState::SignedIn(_))
    }

    /// The signed-in account's own library: playlists (Liked music first, as YouTube Music
    /// lists it), saved albums, followed artists and liked songs. `None` when signed out, or
    /// while it has not loaded. Until the library calls answer, the "Your playlists" and "Liked
    /// songs" shelves of the account's Home stand in; no other shelf does, because those are
    /// suggestions, not the user's library.
    #[must_use]
    pub fn account_lists(&self) -> Option<Arc<AccountLibrary>> {
        if !self.signed_in() {
            return None;
        }
        if let Some(lib) = &self.account_library {
            return Some(Arc::clone(lib));
        }
        let mut lib = AccountLibrary::default();
        for shelf in &self.home.shelves {
            for item in &shelf.items {
                match (shelf.title.as_str(), item) {
                    (YOUR_PLAYLISTS, ShelfItem::Playlist(p)) => lib.playlists.push(p.clone()),
                    (LIKED_SONGS, ShelfItem::Song(t)) => lib.songs.push(t.clone()),
                    _ => {}
                }
            }
        }
        (!(lib.playlists.is_empty() && lib.songs.is_empty())).then(|| Arc::new(lib))
    }

    /// `true` when the song is liked: what the user did this session, else the Liked songs shelf.
    #[must_use]
    pub fn is_liked(&self, track: &Track) -> bool {
        self.likes.get(&track.id).copied().unwrap_or_else(|| {
            if track.origin == nemo_core::audio::Origin::SoundCloud {
                return self.sc_lib.data.likes.iter().any(|t| t.id == track.id);
            }
            self.home
                .shelves
                .iter()
                .filter(|s| s.title == LIKED_SONGS)
                .flat_map(|s| s.items.iter())
                .any(|i| matches!(i, ShelfItem::Song(t) if t.id == track.id))
        })
    }

    /// The tracks of a list.
    #[must_use]
    pub fn tracks_of(&self, source: &ListSource) -> Vec<Track> {
        match source {
            ListSource::Search => self
                .ui
                .search
                .shown()
                .map(|r| r.songs.clone())
                .unwrap_or_default(),
            ListSource::Library => self.local_tracks.clone(),
            ListSource::Daily(n) => self
                .daily_sections
                .get(*n)
                .map(|s| s.tracks.clone())
                .unwrap_or_default(),
            ListSource::Videos => self.daily.videos.clone(),
            ListSource::SearchVideos => self
                .ui
                .search
                .shown()
                .map(|r| r.videos.iter().map(|v| v.track.clone()).collect())
                .unwrap_or_default(),
            ListSource::Explore(n) => self
                .explore
                .shelf(*n)
                .map(crate::explore_feed::shelf_tracks)
                .unwrap_or_default(),
            ListSource::Mood(id, n) => self
                .moods
                .shelf(id, *n)
                .map(crate::explore_feed::shelf_tracks)
                .unwrap_or_default(),
            ListSource::Account => self
                .account_lists()
                .map(|l| l.songs.clone())
                .unwrap_or_default(),
            ListSource::Recent => self.ui.recent.clone(),
            ListSource::ScLikes => {
                // Sorted like the Likes tab draws it, so a row plays from where it is.
                let mut likes = self.sc_lib.data.likes.clone();
                crate::explore::sort_by_key(
                    &mut likes,
                    self.lib_sort,
                    |t| t.title.clone(),
                    |t| crate::format::artists(&t.artists),
                );
                likes
            }
            ListSource::ScSearch => self.sc_group_songs(),
            ListSource::ScHistory(n) => self
                .sc_lib
                .data
                .history
                .get(*n)
                .map(|s| s.tracks.clone())
                .unwrap_or_default(),
            ListSource::Home(n) => self
                .home
                .shelves
                .get(*n)
                .map(|s| {
                    s.items
                        .iter()
                        .filter_map(|i| match i {
                            ShelfItem::Song(t) => Some(t.clone()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            // Album and playlist songs in the order the page shows them (its Sort control).
            ListSource::Detail(key) => match self.details.get(key) {
                Some(DetailState::Ready(d)) => match d.as_ref() {
                    DetailData::Album(a) => sorted(&a.tracks, self.collection_sort(key)),
                    DetailData::Artist(a) => a.top_songs.clone(),
                    DetailData::Playlist(p) => sorted(&p.tracks, self.collection_sort(key)),
                },
                _ => Vec::new(),
            },
        }
    }

    /// Width of the lyrics column on the current screen: the wide full-screen column or the
    /// right pane less its padding.
    pub(crate) fn lyrics_width(&self) -> f32 {
        let (w, h) = self.window_px();
        let px = if matches!(self.ui.route(), Route::NowPlaying) {
            let art = layout::now_playing_art(w, h);
            layout::lyrics_column_width(w, art)
        } else {
            layout::right_width(w)
        };
        // Pane and column widths are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let px = px as f32;
        if matches!(self.ui.route(), Route::NowPlaying) {
            px
        } else {
            px - 2.0 * style::gap::MD
        }
    }
}
