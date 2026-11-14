## Goal

The user can shape the sound when they want to, and only then. NemoMusic gets a ten-band equalizer with presets and headphone profiles from the AutoEq project, delay and reverb effects, and crossfade between songs, including a smart transition in the style of Apple Music's AutoMix. Every one of them is off by default. When one is on, the signal-path view lists it and the quality badge stops claiming a bit-perfect path. When all are off, the audio path does no extra arithmetic and the output stays bit-exact.

## User Stories

- As a listener with headphones, I want a ten-band equalizer with presets, so that I can fix or colour the sound.
- As a listener with a known headphone model, I want to load its AutoEq profile, so that I do not tune by ear.
- As a listener, I want delay and reverb, so that I can add space to dry recordings.
- As a listener who plays playlists, I want songs to blend into each other, so that the music flows.
- As an audiophile, I want every effect off until I turn it on, so that the default path stays untouched.
- As an audiophile, I want to see each active effect in the signal path, so that I know what changed the sound.

## Acceptance Criteria

1. THE SYSTEM SHALL ship with the equalizer, delay, reverb and crossfade all off, and SHALL keep them off after an update.
2. WHEN the user turns an effect on or off, THE SYSTEM SHALL apply it to the playing song within 100 ms without a click, a pop or a restart.
3. WHILE every effect is off, THE SYSTEM SHALL skip the effect stages with no arithmetic, and the bit-exact test (known PCM in, identical samples out) SHALL pass.
4. WHILE an effect is on, THE SYSTEM SHALL list it in the signal-path view with its settings (for example "Equalizer, preset Flat, preamp -3 dB"), and SHALL NOT show the path as bit-perfect or the file as true-lossless.
5. THE SYSTEM SHALL offer a ten-band equalizer with one gain per band, from -12 dB to +12 dB, at the standard ISO octave centres from 31 Hz to 16 kHz.
6. THE SYSTEM SHALL offer a preamp, and WHEN the combined gain could push samples over full scale, THE SYSTEM SHALL lower the preamp automatically or limit the peaks, and SHALL show which one it did.
7. IF the output still exceeds full scale, THEN THE SYSTEM SHALL count it, SHALL NOT wrap around, and SHALL show a clipping warning in the signal path.
8. THE SYSTEM SHALL offer built-in presets (at least Flat, Bass boost, Treble boost, Vocal) and SHALL let the user save, rename, delete, export and import their own presets as files.
9. WHEN the user picks "Flat" or resets the equalizer, THE SYSTEM SHALL set every band and the preamp to 0 dB.
10. WHEN the user loads an AutoEq profile, THE SYSTEM SHALL read the fixed-band (ten-band) result or a parametric result, apply its preamp, and show the profile name and its measurement source (for example the measurer's name).
11. IF a loaded profile has filters the equalizer cannot represent, THEN THE SYSTEM SHALL say how it was approximated or refuse it, and SHALL NOT apply it silently.
12. THE SYSTEM SHALL let the user pick an AutoEq profile from a bundled list or from a file the user chooses, and SHALL contact no network host for this unless the owner approves one (Open Question 2).
13. THE SYSTEM SHALL credit the AutoEq project and each data source in the app's licence and about screen, and SHALL follow the licence terms of the code and the data it bundles.
14. WHEN delay is on, THE SYSTEM SHALL let the user set time, feedback and wet/dry mix, and WHEN reverb is on, size, damping and wet/dry mix, each with a safe limit so the effect cannot grow without bound.
15. WHEN the user sets delay or reverb, THE SYSTEM SHALL let the tail ring out at the end of a song or a stop without a click.
16. WHEN crossfade is on, THE SYSTEM SHALL overlap the end of one song with the start of the next for a length the user sets from 1 to 12 seconds.
17. WHEN the user skips to the next song by hand, THE SYSTEM SHALL play a short fade of at most 1 second or no fade, as the user chose, and SHALL NOT wait for the full crossfade length.
18. WHILE two songs on one album are marked gapless (a live album, a mix), THE SYSTEM SHALL play them gapless and SHALL NOT crossfade between them, unless the user turned that exception off.
19. WHEN the user turns on the smart transition, THE SYSTEM SHALL choose per song pair between a plain crossfade, trimming silence at the end and start, and a plain gapless join, and SHALL show which one it used.
20. IF the smart transition cannot analyse a song (for example a stream with no lead time), THEN THE SYSTEM SHALL fall back to the plain crossfade or a gapless join.
21. WHILE a crossfade or transition runs, THE SYSTEM SHALL show it in the signal path and SHALL show the real playing song in the player bar.
22. WHEN the next song has another sample rate or channel count, THE SYSTEM SHALL still crossfade, and SHALL show any resampling it needed.
23. THE SYSTEM SHALL run every effect without allocation, locks, blocking calls or logging inside the audio callback, proved by the counting-allocator test of code.md, and SHALL cover the effects in that test.
24. WHILE effects run on the reference machine, THE SYSTEM SHALL keep the audio callback under 25% of its period and SHALL NOT raise the underrun count.
25. THE SYSTEM SHALL let the user reach every control with the keyboard, with a visible focus, and SHALL name the bands and sliders for screen readers.
26. WHEN reduced motion is set, THE SYSTEM SHALL draw the equalizer curve and level meters without animation.
27. THE SYSTEM SHALL keep effect settings on this computer, and SHALL contact no new host.

## Out of Scope

- Effects on the system or on other apps (a system-wide equalizer).
- Room correction, spatial audio, upmixing, loudness war "enhancers".
- Beat matching and tempo change of songs (Open Question 4).
- Per-device or per-song equalizer profiles.
- Plugin hosts (VST, LV2) and recording.
- Android or phone equalizers.

## Open Questions

These need the owner (workflow.md section 8, coc.md):

1. Licence. AutoEq's code is MIT (verified from its LICENSE file). Its measurement data and result files come from several measurers (oratory1990, Crinacle, Innerfidelity, Rtings, others) and I found no data licence statement; a maintainer comment says one measurer asked that its numbers not be published in the repo. Treat the data licence as unverified. May NemoMusic bundle result files, or must the user bring a file?
2. Network. Fetch profiles from GitHub at the user's request (adds a host to the README egress list), or bundle a list, or file import only?
3. Apple Music's crossfade is a user-set 1 to 12 seconds. AutoMix chooses a transition per song pair and may trim silence or crossfade (Apple support pages). Apple's own pages do not say how it works inside; does the owner want the three-way choice of criterion 19, or only plain crossfade?
4. Beat matching and time stretching: out of scope for the first release, or a later spec? It needs tempo analysis (cost and licence of any analysis crate unverified).
5. Technical, for design: the YouTube stream arrives only as the current song plus a prefetch. How long a lead time before the end is needed, and does the next song's decode start early enough for the longest crossfade?
6. Technical, for design: floating-point or integer processing, and limiter versus automatic preamp (criterion 6).
7. Does the equalizer apply to local lossless files too? Assumed yes, with the true-lossless badge removed while it is on.

<!-- Sources:
.kiro/steering/product.md (principle 1, Quality badge rule), code.md (Real-time audio thread, Bit-perfect), coc.md; crates/nemo-core/src/audio.rs (ProcessingStage::Equalizer, is_bit_perfect); crates/nemo-audio (gapless, volume)
https://github.com/jaakkopasanen/AutoEq (project)
https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/LICENSE (MIT, code)
https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/README.md (results from several measurers)
https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/oratory1990/over-ear/1MORE%20SonoFlow/README.md (parametric and fixed-band tables: ten bands 31 Hz to 16 kHz, Q 1.41, with preamp)
https://github.com/jaakkopasanen/AutoEq/issues/235 (a measurer asked that its data not be published)
https://support.apple.com/en-us/105067 (AutoMix and Crossfade)
https://support.apple.com/en-in/guide/iphone/iphadf2fe1f4/ios (AutoMix chooses the transition)
https://support.apple.com/en-au/guide/music/muse5e9ec085/mac (Crossfade seconds slider)
https://discussions.apple.com/thread/256202907 (1 to 12 seconds)
-->
