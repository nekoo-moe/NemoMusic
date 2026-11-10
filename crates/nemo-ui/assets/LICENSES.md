# Bundled assets

| Asset | Source | Licence |
|---|---|---|
| `fonts/GoogleSansFlex-Regular.ttf`, `-Medium.ttf`, `-SemiBold.ttf` | Google Sans Flex, https://github.com/googlefonts/googlesans-flex, as published on Google Fonts (`ofl/googlesansflex` in https://github.com/google/fonts) | SIL Open Font License 1.1, full text in `fonts/OFL.txt` (Copyright 2015 The Google Sans Flex Authors). No Reserved Font Name is declared. |
| `icons/*.svg` | Material Symbols (Rounded, weight 400, grade 0, 24 px; `-fill` files are the filled variants), https://github.com/google/material-design-icons (`symbols/web`) | Apache License 2.0, text in `icons/LICENSE-APACHE-2.0.txt` |

## How the font files were made (OFL "Modified Version" notice)

The published font is one variable file with five axes (weight, width, optical size, slant,
roundness). iced 0.14 cannot pick a variable-font instance, so three static instances were cut
with `fonttools varLib.instancer` (fonttools 4.62.0): `wght` 400 / 500 / 600, `wdth` 100,
`opsz` 18, `slnt` 0, `ROND` 0. Two more changes, both allowed by the OFL because no Reserved Font
Name exists:

- The default digits `0` to `9` were mapped to the font's own tabular-figure glyphs (the `tnum`
  feature), because iced 0.14 cannot switch OpenType features on. Times, rates and counts keep a
  fixed width, so nothing shifts while a timer ticks.
- The family name stays "Google Sans Flex"; weights 500 and 600 carry the typographic family
  (name ID 16) "Google Sans Flex" with the style names Medium and SemiBold.

Vietnamese is covered (all of U+1EA0 to U+1EF9 and the Latin Extended-A/B letters); the snapshot
`docs/ui/snapshots/specimen-vietnamese.png` shows "Mới Quen Nhau Đã Rủ" and "HIEUTHUHAI, Phương Ly".
CJK and right-to-left text fall back to system fonts.
