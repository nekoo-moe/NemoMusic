## Goal

The user picks which speakers or headphones NemoMusic plays on, from a list on screen, and NemoMusic uses only that device. Other apps keep their own audio and the system default stays as it was. The user can switch device while a song plays, and the choice is remembered. On Windows, NemoMusic can open the device in WASAPI exclusive mode, so the path is bit-perfect when the device accepts the song's format. When it cannot, NemoMusic falls back to shared mode and says so.

## User Stories

- As a listener with several outputs, I want to pick the device NemoMusic uses, so that music goes to my DAC and system sounds stay on the speakers.
- As a listener, I want to switch device while a song plays, so that I do not restart the app or the song.
- As a listener, I want NemoMusic to remember my device, so that I pick it once.
- As a listener, I want a clear message when my headphones are unplugged, so that music does not blast from the speakers.
- As an audiophile on Windows, I want exclusive mode, so that Windows does not mix or resample my music.
- As an audiophile, I want an honest answer when exclusive mode did not work, so that I do not think the path is bit-perfect when it is not.

## Acceptance Criteria

1. WHEN the user opens the output setting, THE SYSTEM SHALL list every output device of the system, mark the system default, and show the saved choice.
2. WHEN the user picks a device, THE SYSTEM SHALL play on that device only, and SHALL NOT change the system default device or the volume or routing of other apps.
3. WHILE no device is chosen, THE SYSTEM SHALL use the system default device, and SHALL follow the default when it changes.
4. WHEN the user switches device during playback, THE SYSTEM SHALL continue the same song from the same position on the new device, with a gap of at most 500 ms and no click, and SHALL keep the queue.
5. IF the new device cannot be opened, THEN THE SYSTEM SHALL keep playing on the old device, say why, and SHALL NOT change the saved choice.
6. WHEN the user picks a device, THE SYSTEM SHALL save it on this computer and use it at the next start.
7. THE SYSTEM SHALL save the device by a stable identifier plus its name, not by name only, and SHALL treat two devices with the same name as two devices (the design picks the identifier).
8. IF the saved device is not present at start, THEN THE SYSTEM SHALL start on the system default device, say so, show the saved choice as "not connected", and return to it when it appears.
9. WHEN the device in use is unplugged, disabled or fails during playback, THE SYSTEM SHALL pause within 500 ms, say which device was lost, and SHALL NOT move to another device without the user's action.
10. WHEN the lost device returns, THE SYSTEM SHALL offer to resume. It SHALL resume by itself only if the user turned that on, and that setting SHALL be off by default.
11. WHEN a device is added or removed, THE SYSTEM SHALL update the list without a restart.
12. WHILE the user has not turned exclusive mode on, THE SYSTEM SHALL open devices in shared mode and label the path as shared mode, never as bit-perfect.
13. THE SYSTEM SHALL offer exclusive mode on Windows only, as a setting that is off by default, with a short text saying that other apps cannot use the device while it is on.
14. WHEN exclusive mode is on and a song starts, THE SYSTEM SHALL ask the device for the song's own sample rate, channel count and bit depth first, and SHALL show the negotiated format in the signal path.
15. IF the device does not accept the song's format in exclusive mode, THEN THE SYSTEM SHALL try the nearest format the device accepts, show that resampling or conversion is on, and SHALL NOT label the path bit-perfect.
16. IF exclusive mode cannot be opened (device in use by another app, driver refuses, the Windows setting that lets apps take exclusive control is off), THEN THE SYSTEM SHALL fall back to shared mode, keep playing, and show the reason in plain words.
17. THE SYSTEM SHALL label the path bit-perfect only when the device is open in exclusive mode, no effect or volume stage is on, and the device format equals the decoded format.
18. WHEN the song changes to one with another format and the device is in exclusive mode, THE SYSTEM SHALL reopen the device in the new format without a click, and SHALL keep gapless playback where the formats match.
19. WHILE exclusive mode is on, THE SYSTEM SHALL leave the volume to the device and the OS as today, and SHALL NOT add software volume.
20. WHEN playback is paused or stopped, THE SYSTEM SHALL release an exclusive device after a delay the user can set (default 5 seconds), so other apps can use it.
21. IF another app takes the exclusive device away during playback, THEN THE SYSTEM SHALL pause, say so, and offer to retry.
22. THE SYSTEM SHALL run the output callback without allocation, locks, blocking calls or logging, in shared and in exclusive mode (code.md), and the bit-exact test SHALL cover the exclusive path.
23. WHILE exclusive mode is on, THE SYSTEM SHALL use a buffer of 20 ms or less by default, as code.md asks, and SHALL let the user raise it when the device underruns.
24. THE SYSTEM SHALL let the user reach the device list, the exclusive setting and the retry action with the keyboard, with a visible focus.
25. WHEN the device list or a message appears, THE SYSTEM SHALL follow the reduced-motion setting.
26. THE SYSTEM SHALL NOT write device names or identifiers to logs at info level or above, and SHALL contact no network host for this feature.
27. WHEN the app runs on Linux, THE SYSTEM SHALL offer the device picker in shared mode, show no exclusive setting, and say the exclusive path is planned.

## Out of Scope

- Direct ALSA `hw:` output on Linux (product.md, "Later"), and CoreAudio on macOS.
- ASIO (a separate optional feature in tech.md).
- Casting to network devices (a separate `cast` spec).
- A system-wide mixer, per-app volume, and routing other apps.
- Surround and speaker layouts above stereo.
- Software volume in the bit-perfect path.

## Open Questions

These need the owner (workflow.md section 8, coc.md):

1. Should the player resume by itself when a lost device returns (criterion 10)? Default proposed: off.
2. Exclusive-mode default after the user turns it on: fall back to shared at once (proposed), or ask each time?
3. Does the owner accept the idle release delay of 5 seconds (criterion 20), or want the device held while the app is open?
4. Technical, for design: whether `cpal` 0.18.2 can open WASAPI exclusive mode (unverified; tech.md already says it is unverified) or the `wasapi` crate 0.27.0 is needed for the whole Windows path. The `wasapi` README lists shared and exclusive modes, event-driven buffering, device-change notifications and an exclusive-mode format probe; its MSRV is 1.85 and it uses edition 2024.
5. Technical, for design: the device identifier to save. `cpal` today matches by name only (`open_named`), which fails for two identical devices.
6. Technical, for design: unsafe code. The `wasapi` crate presents a safe API; if the design needs raw calls, they must sit in an `ffi` or `sys` module of `nemo-audio` (code.md, Unsafe).
7. Technical, for design: which exclusive formats to try, in what order, and whether 24-bit in a 32-bit container counts as the same bit depth for criterion 17.
8. Reference hardware for the exclusive-mode test (product.md, Open decision 7).

<!-- Sources:
.kiro/steering/product.md (MVP, Later, Quality badge rule, Open decision 7), tech.md (Device output, Exclusive mode, ASIO), code.md (Real-time audio thread, Bit-perfect, Unsafe), coc.md (User Privacy)
crates/nemo-audio/src/cpal_sink.rs (list_output_devices, CpalSink::open_named, OutputMode::Shared); crates/nemo-app/src/config.rs (output_device); crates/nemo-core/src/audio.rs (OutputMode, is_bit_perfect); .kiro/specs/app-mvp/tasks.md (4.2 WASAPI exclusive)
https://raw.githubusercontent.com/HEnquist/wasapi-rs/master/README.md (wasapi crate features)
https://learn.microsoft.com/en-us/windows/win32/coreaudio/exclusive-mode-streams (exclusive mode)
https://github.com/HEnquist/camilladsp/blob/master/backend_wasapi.md (exclusive mode, one app at a time)
-->
