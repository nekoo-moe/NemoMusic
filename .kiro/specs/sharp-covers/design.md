# Sharp covers: design

Spec approved by the owner on 2026-11-15.

## Overview

Root causes found in the code on `main` (2026-11-15):
- `nemo-ui` `ArtSpec::px` asked for 1.5 x the slot, capped at 480, whatever the real scale factor; the Now Playing cover asked for 640 and baked 480. At 200 % a 160 px card got 240 px drawn on 320 device px, and the 480 px Now Playing art got 480 px on 960.
- The app never read the window scale factor.
- `nemo-ytm` `ImageCache` fetched the listed thumbnail as is. YouTube Music lists songs only at 60 and 120 px (`=w60-h60`, `=w120-h120`) and albums at up to 544, so even a correct request was capped at the largest listed size and the GPU stretched it.
- `cover::bake` used the Triangle (bilinear) filter, which blurs fine detail when it shrinks.

Owner answers (2026-11-15): no `-nu` option; a source smaller than its slot is drawn at native size, centred; old cache files are pruned once.

Fix: the UI asks for slot x scale device px; the image cache rounds that to a few steps and rewrites Google image size suffixes or picks a larger `i.ytimg.com` variant; the bake cuts video letterboxes and shrinks with Lanczos3. All decode and resize stays in jobs on the iced thread pool, never on the UI thread.

## Affected Crates

- `nemo-ytm`: `src/images.rs`, new `src/images/variant.rs`, `src/images/tests.rs`.
- `nemo-ui`: `src/gui/app/{art,query,job,dispatch,message,handle,update}.rs`, `src/gui/app.rs`, `src/gui/mod.rs`, `src/gui/headless.rs`, `src/cover.rs`, new `src/cover/letterbox.rs`, `src/mock/cover.rs`.
- `nemo-core`: not changed. `ImageSource::image(artwork, target_px)` keeps its signature; `target_px` is now device px.

## Data Flow

1. iced startup task `window::scale_factor` and the `window::Event::Rescaled` event send `Message::Rescaled(f32)`. `App.scale_pct` stores it in percent (50 to 400).
2. `App::art_handle` builds an `ArtSpec { artwork, size, scale_pct, shape }`; the key includes the scale, so a new scale gives new entries and the LRU drops the old ones.
3. `Job::Art` and `Job::Cover(track, scale_pct)` run on the iced thread pool: `ImageSource::image(artwork, device_px)`, then `cover::bake(bytes, device_px, fraction)` (letterbox strip, square crop, Lanczos3 shrink, corner cut). `Message::Cover` carries the scale; a bake for an older scale is dropped.
4. `ImageCache::load` (tokio runtime of `nemo-ytm`): `pick_thumbnail` chooses the listed size, `variant::candidates` lists sized addresses plus the listed one, each is checked against the allow-list, and `Inner::remote` tries them in order: disk cache, `<hash>.missing` marker, download.

5. Drawing (`kit::fitted_art`, used by `kit::art` and the cover slots in `bar::cover_art`): `art_fit(source_px, slot, scale_pct)` decides `Fill` (source >= ceil(slot x scale)) or `Native(logical px)`; native pictures sit centred in a `surface-container` box of the slot size.
6. Cache format: file names are `v2-<sha256 hex>` (plus `.missing`, `.tmp`). `ImageCache::new` starts `prune::prune_old_files` with `spawn_blocking` on the `nemo-ytm` runtime: without the `format-v2` marker it deletes regular files in the cache folder whose names are not of that format, then writes the marker. The folder is `cache/images`, used by nothing else.

No audio buffers or audio threads are involved.

## Interfaces

```rust
// nemo-ytm::images::variant
pub const SIZE_STEPS: [u32; 4];
pub fn size_step(px: u32) -> u32;
pub fn candidates(url: &str, px: u32) -> Vec<String>;

// nemo-ytm::images::prune
pub const FILE_PREFIX: &str;
pub const MARKER: &str;
pub fn is_current_name(name: &str) -> bool;
pub fn prune_old_files(dir: &Path) -> std::io::Result<usize>;

// nemo-ui::gui::app::art
pub const MAX_ART_PX: u32;
pub fn scale_pct(factor: f32) -> u32;
pub fn device_px(slot: u32, scale_pct: u32) -> u32;
pub struct ArtSpec { pub artwork: Artwork, pub size: u32, pub scale_pct: u32, pub shape: ArtShape }
pub enum ArtFit { Fill, Native(f32) }
pub fn art_fit(source_px: u32, slot: u32, scale_pct: u32) -> ArtFit;
pub fn handle_px(handle: &image::Handle) -> Option<u32>;
// nemo-ui::gui::views::kit
pub fn fitted_art<'a>(handle: image::Handle, size: f32, radius: f32, scale_pct: u32, opacity: f32) -> Element<'a, Message>;

// nemo-ui::gui::app::Message
Rescaled(f32),
Cover { track: TrackId, scale: u32, built: Result<Arc<CoverBuilt>, String> },
// nemo-ui::gui::app::Job
Cover(Track, u32),
```

## Error Handling

- A 404 or 410 on a sized guess: marker file, next address. Another error status or a non-image answer: next address. A transport error: stop and return it. When every address fails, the error of the last one is returned (the listed address keeps its old behaviour, including no marker).
- Marker and cache writes are best effort; a lost marker costs one request later.
- Unknown URL shapes are used as listed.
- Pruning: a file that cannot be deleted is skipped; if the folder cannot be listed or the marker cannot be written, nothing is recorded and the prune runs again at the next start. The result is ignored (it only frees space).

## Testing Strategy

- `nemo-ytm` `images::variant::tests`: `cover_url_asks_for_the_slot_size_times_scale`, step rounding, banner aspect, `fcrop64` options kept, unsized and unknown addresses left alone, `i.ytimg.com` order.
- `nemo-ytm` `images::tests`: `a_missing_sized_address_falls_back_and_is_not_asked_for_again`, `the_listed_address_is_the_last_resort_and_its_error_is_reported` (loopback test server, no real host).
- `nemo-ui` `gui::app::art::tests`: `art_asks_for_the_slot_size_times_the_window_scale`, cache key per scale, odd scale factors.
- `nemo-ui` `cover::letterbox::tests` and `cover::tests::video_thumbnail_bakes_to_a_square_of_the_picture_without_bars`.
- `nemo-ytm` `images::prune::tests`: `old_cover_files_are_pruned_once_and_new_ones_kept` (temp dir: old names and a stray file go, new names, the marker and a folder stay, the second run is a no-op), name format, missing folder.
- `nemo-ui` `gui::app::art::tests::a_source_at_least_the_slot_fills_it_and_a_smaller_one_is_drawn_native_and_centred`.
- Headless snapshots now run at the `iced_test` scale of 2.0 (`Message::Rescaled` in `build_app`).
- No bit-exact test: audio is not touched.

## Risks and Alternatives

- A step bigger than the original makes the Google image server scale the image itself (unverified, see Open Questions).
- More bytes per cover at high scale (a 1080 px JPEG is about 100 to 200 KB); the step list keeps at most four sizes per cover.
- Alternative: ask for the exact device size. Rejected: one cache file per window size and scale.
- Alternative: detect letterbox bars on any image. Rejected: dark square covers would be cut.
