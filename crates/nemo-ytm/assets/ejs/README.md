# EJS solver scripts

`yt.solver.core.min.js` and `yt.solver.lib.min.js` are the release assets of version 0.8.0 of
`github.com/yt-dlp/ejs` (tag `0.8.0`, `https://github.com/yt-dlp/ejs/releases/download/0.8.0/`),
unchanged. Licence: Unlicense (public domain, see the header of each file); `lib` bundles
meriyah 6.1.4 and astring 1.9.0 (ISC, their notices are in the file).

| File | SHA-256 |
|---|---|
| `yt.solver.core.min.js` | `18da6ce0758b416e7ae645084f4f8801f9f9d59d6c477c05eaa0ff94ebd8cc00` |
| `yt.solver.lib.min.js` | `c55987fe697e5b9ee18830163f7af85327e9bb5c3e674b969d38c8d205eaa577` |

To update: download both assets of a newer release, replace the files, run
`cargo test -p nemo-ytm --lib cipher::tests::vectors -- --ignored --nocapture` (it checks every
test vector of the EJS repository against real player scripts) and record the new hashes here.
The scripts are only ever run inside the embedded engine of `nemo-ytm::cipher`, which has no
file, network or module access.
