# Bundled assets

| Asset | Source | Licence |
|---|---|---|
| `fonts/IBMPlexSans-Regular.ttf`, `fonts/IBMPlexSans-SemiBold.ttf` | IBM Plex Sans, https://github.com/IBM/plex (`packages/plex-sans/fonts/complete/ttf`) | SIL Open Font License 1.1, text in `fonts/OFL.txt` (Copyright IBM Corp., Reserved Font Name "Plex") |
| `icons/*.svg` | Drawn for NemoMusic: one stroke style, 24 px grid, 1.5 px stroke | Original work, same licence as the project; no third-party artwork |

Digits in IBM Plex Sans are tabular by default (all ten advance 600/1000 em); a unit test in `src/gui/assets.rs` reads the font tables to check it. No Material Symbols are bundled.
