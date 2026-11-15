# Sharp covers: requirements

Spec approved by the owner on 2026-11-15 (pre-approved in the owner report of that day).

## Goal

Cover art looks sharp everywhere in the app (cards, rows, hero, Now Playing, the player bar, the sidebar), at 100 % and at 125 to 200 % display scale. Each picture is fetched and decoded at the slot size times the window scale factor, so one source pixel lands on one screen pixel, and no cover is upscaled by our own resize.

## User Stories

- As a listener on a 150 % or 200 % laptop screen, I want covers as crisp as the text around them, so that the app looks finished.
- As a listener, I want the disk cache to stay small, so that sharp covers do not fill my disk.
- As a listener with a slow connection, I want a missing large variant to fall back quickly, so that covers still appear.

## Acceptance Criteria

1. WHEN a view asks for a picture in a slot of S logical px and the window scale factor is F, THE SYSTEM SHALL fetch and decode it for ceil(S x F) device px, at most 1080.
2. WHEN the window scale factor changes (another screen, OS setting), THE SYSTEM SHALL load the pictures and the current cover again for the new scale, without a cross-fade for the same cover.
3. WHEN the image cache fetches remote art, THE SYSTEM SHALL round the device size up to one of the steps 120, 240, 544, 1080 (the largest step when it is bigger).
4. WHEN the chosen address is on `*.googleusercontent.com` or `*.ggpht.com` and carries a size suffix (`=w120-h120-...`, `=s120`), THE SYSTEM SHALL ask for the step size by rewriting that suffix, keeping the aspect of a `w`/`h` pair and every other option.
5. WHEN the chosen address is an `i.ytimg.com/vi/<id>/<variant>` video thumbnail, THE SYSTEM SHALL try the smallest variant whose 1:1 crop covers the step first, then larger variants, then smaller ones, and the listed address last.
6. WHEN a sized address answers 404 or 410, THE SYSTEM SHALL try the next address and remember the miss on disk, so it is not asked for again; the listed address is never marked.
7. WHEN a request fails without an answer (offline, timeout), THE SYSTEM SHALL stop trying more addresses for that picture.
8. WHEN a 4:3 video thumbnail has black letterbox bars (top and bottom eighths), THE SYSTEM SHALL cut them off before the 1:1 `cover` crop.
9. WHEN a picture is larger than the device size, THE SYSTEM SHALL shrink it with a Lanczos3 filter on a worker thread; WHEN it is smaller, THE SYSTEM SHALL keep its own size (no upscale in our resize).
10. THE SYSTEM SHALL contact no host that is not already on the image allow-list.

## Out of Scope

- Drawing a too-small source centred at native size instead of stretching it to the slot (design.md section 9 alternative); left as an owner question.
- Pruning old cache files of the previous size scheme.
- Local file art (it is read as is).

## Open Questions

- Whether `lh3.googleusercontent.com` upscales a small original when asked for a larger `w`/`h`/`s`. YouTube Music originals are normally well above 1080 px, so this is not expected, but it is not verified (no live requests in tests).
- design.md section 9 asks to centre a source smaller than the slot at native size; today the image widget stretches it (`ContentFit::Fill`). Owner to decide whether to change that draw step.
