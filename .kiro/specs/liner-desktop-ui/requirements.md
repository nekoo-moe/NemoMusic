# Liner-style Desktop UI Refinement: Requirements

Owner request: 2026-11-16.
Follows design inspirations from Liner Desktop (`tryliner/desktop`) and addresses visual feedback on Synced Lyrics, Player Bar, and Window Backdrop.

## Goal

Modernize and mature the desktop visual experience by refining Synced Lyrics typography to clean, non-bold, high-contrast monochrome (White active / Grey upcoming, eliminating childish accent glow), replacing the floating Liquid Glass capsule player bar with a sleek, full-width docked horizontal music bar inspired by Liner Desktop, and enriching the window background with a subtle, atmospheric ambient glow derived from album artwork.

## User Stories

- As a listener, I want Synced Lyrics (line-timed) to display in a mature, non-bold sans font with pure white for the active line and grey for upcoming lines, without colorful glow effects, so that reading lyrics feels calm and distraction-free.
- As a listener, I want a docked, full-width horizontal music player bar at the bottom (similar to Liner Desktop) without floating glass margins or pill shapes, so that screen space is used efficiently and transport controls feel anchored and modern.
- As a listener, I want a deeper, more atmospheric ambient background glow reflecting the current track's artwork (similar to Liner Desktop), so that the application feels immersive while maintaining perfect readability.

## Acceptance Criteria

1. WHEN line-synced lyrics (`Granularity::Line`, such as LRCLIB or `.lrc`) are rendered:
   - THE SYSTEM SHALL render the active lyric line using a clean, non-bold font weight (regular 400).
   - THE SYSTEM SHALL render the active line in pure bright white (`on-surface` / `#FFFFFF`) and upcoming/inactive lines in neutral grey (`on-surface-variant` with appropriate opacity).
   - THE SYSTEM SHALL NOT render colorful accent glows (`glow_layers` with accent tint) behind line-synced lyrics.
   - THE SYSTEM SHALL preserve existing Syllable Lyrics (`Granularity::Word` / `Syllable`) motion and styling untouched.
2. WHEN the horizontal music player bar is rendered:
   - THE SYSTEM SHALL dock the player bar to the bottom edge of the window spanning full width (`Length::Fill`), removing the floating capsule margin, heavy glass shadow, and capsule border radius.
   - THE SYSTEM SHALL organize controls into three balanced zones:
     - Left: Album cover art (subtle rounded corners `shape.sm`), track title, artist name, and Like (heart) button.
     - Center: Playback transport controls (Previous, circular Play/Pause button, Next) and integrated progress/scrubber bar with elapsed and remaining timestamps.
     - Right: Utility controls (Shuffle, Repeat, Lyrics toggle, Volume slider, Output device/Cast, and Queue/Side panel toggle).
   - THE SYSTEM SHALL render the bar on a clean dark surface (`surface-container` / `surface-container-low`) with a 1 px subtle top border (`outline-variant`), eliminating the simulated liquid glass refraction and pill frame.
3. WHEN the window background is rendered:
   - THE SYSTEM SHALL generate a rich, subtle ambient background glow (ambient field) derived from the playing track's artwork palette, smoothly blending into the deep dark base surface.
   - THE SYSTEM SHALL guarantee text contrast ratio >= 4.5:1 (WCAG AA) across all panes and controls over the ambient background.
   - WHILE reduced motion or reduced transparency is enabled, THE SYSTEM SHALL fall back to a calm, static, flat tonal background without GPU overhead.

## Out of Scope

- Modifying the underlying audio engine, real-time audio threads, decoders, or bit-perfect output pipeline.
- Modifying word/syllable-level animation logic in `syllable_text.rs` (which the owner confirmed is already working well).
- Remote network streaming or backend catalog services.

## Open Questions

- Should the horizontal progress scrubber bar sit inside the center controls column (like standard desktop layouts) or span full-width across the top edge of the docked bottom bar? (Proposed: centered in the middle zone for compact elegance, with option for full-width hairline progress).
