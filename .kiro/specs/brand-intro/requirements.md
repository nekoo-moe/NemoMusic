# Brand intro, rebuilt setup and folder picker: requirements

Owner approved the request 2026-11-19: "Redo the loading animation when the app opens, make it cooler and mark the brand." "Redo the UI and the animation of the first-time setup." "Add crates whenever they make the app better." Related specs: `startup-flow` (this one replaces its splash look and setup layout, and finishes its task 6), `app-motion`.

## Goal

The app opens with a short, confident brand intro: the mark, the name and one line about the product, over the real start-up progress. The first-run setup becomes a large, calm stage with a brand panel, a stepper and one content card, with motion that follows the app's motion rules. `Add a folder` in the setup and in the library opens the operating system's folder dialog.

## User Stories

- As a person opening NemoMusic, I want the start to feel like a product with a face, so that the wait before the app is ready is pleasant and short.
- As a new user, I want the setup to show where I am and what is next, and to fit my small window without scrolling, so that I finish it in a minute.
- As someone who prefers less motion, I want the intro and the setup to hold still, so that nothing moves that I did not ask for.
- As a user with local music, I want to pick a folder with the usual system dialog, so that I do not type a path.

## Acceptance Criteria

Brand intro (PR 1):
1. WHEN the window opens, THE SYSTEM SHALL draw on the first frame the neutral surface of the saved base mode and then play the intro: the mark (a circle in `primary` with the note glyph in `on-primary`) scales in with one overshoot of at most 12 %, the note glyph rises 12 px into place while it fades in, and a soft radial bloom of the accent grows behind the mark and then breathes in the 2.6 s cycle of the heartbeat.
2. WHEN the mark has settled, THE SYSTEM SHALL reveal the wordmark `NemoMusic` in Inter Display 600 letter by letter, each letter 35 ms after the one before, rising 8 px and fading in over `motion.base`, and then the tagline `A quiet room for your music.` in `body-md`.
3. THE SYSTEM SHALL keep the real start-up progress line and the running step name in `label-sm` at the bottom, with no percentage and no spinner.
4. THE SYSTEM SHALL keep the splash on screen for at least 1.2 s, and no longer than the real start-up needs after that; WHEN the services are not ready after 6 s, THE SYSTEM SHALL add `This is taking longer than usual.` as before.
5. WHEN the app is ready and the floor is over, THE SYSTEM SHALL shrink the mark to 0.9 and fade the whole splash over `motion.base` on the emphasized-accelerate curve, and THEN draw no more frames for it.
6. WHEN Reduce motion is on, THE SYSTEM SHALL draw the mark, the wordmark and the tagline at rest from the first frame, with no bloom breathing, and remove the splash at once after a floor of 500 ms.
7. WHEN the start fails, THE SYSTEM SHALL show the mark and the wordmark at rest, the plain sentence and a `Close` button, and draw no frames.
8. THE SYSTEM SHALL draw the intro with iced shapes, the bundled fonts and one generated radial texture, and every text ink SHALL keep 4.5:1 on the dark, AMOLED and light surfaces.

Setup layout and motion (PR 2 and 3):
9. WHEN the window is at least 1100 px wide, THE SYSTEM SHALL draw the setup as a left brand panel (the mark, the step title in `font.display`, a vertical stepper of the six steps with the current one highlighted and the finished ones checked) and a right content card; below 1100 px the stepper SHALL be a top row of dots and the brand panel SHALL shrink to the mark.
10. THE SYSTEM SHALL show on each step one headline, one sentence and then the controls, and SHALL fit every step at 960 x 600 without scrolling, except that the card MAY scroll when the controls cannot fit at a larger text scale.
11. WHILE the setup is open, THE SYSTEM SHALL keep previewing the choices live in the window (base, accent, corners, Now Playing style, song colours).
12. WHEN the step changes, THE SYSTEM SHALL slide the card content 24 px with a fade over `motion.base` (emphasized-decelerate), and the rows of the new step SHALL rise in with a 40 ms stagger; no transition SHALL last more than 400 ms.
13. WHEN a step is finished, THE SYSTEM SHALL morph its stepper marker into a check that draws over `motion.base`; buttons SHALL have hover and pressed states; the Done step SHALL play one small settle of the mark, once.
14. WHEN Reduce motion is on, THE SYSTEM SHALL change steps, markers and rows at once, and draw no frames while the setup is idle.
15. THE SYSTEM SHALL keep every behaviour of the current flow: `Skip setup` always available, the notice box unticked by default with Continue disabled until it is ticked, `Not now` allowed, sign-in through the existing account flow, the SoundCloud card honest about what exists, `Run setup again` in Settings without resetting sign-in or data, and no setup for existing users.

Folder picker (PR 4):
16. WHEN the user presses `Add a folder`, THE SYSTEM SHALL open the operating system's folder dialog without blocking the UI thread, and WHEN a folder is chosen, hand it to the existing library folder API and show it in the Local music card and in the Library settings.
17. WHEN the dialog is cancelled, THE SYSTEM SHALL change nothing.
18. THE SYSTEM SHALL NOT write a folder path or file name to a log at `info` level or above; logs carry counts only.

## Out of Scope

- A shared-element glide of the mark to the sidebar logo (the exit shrinks and fades instead).
- New image or font assets; sound; a video or Lottie intro.
- A SoundCloud sign-in, and the Apple Music Now Playing layout (startup-flow tasks 5 and 7).
- Changing what the setup stores or the order of its six steps.
- Window glass, the player bar, skeletons, lyrics and audio.

## Open Questions

- The brief calls the mark a pink circle; the sidebar logo is a 36 px rounded square in `primary`. The intro uses a circle. Should the sidebar logo become a circle too? (Not changed here.)
- The splash exit shrinks and fades instead of gliding to the sidebar logo. Is a glide wanted later?
