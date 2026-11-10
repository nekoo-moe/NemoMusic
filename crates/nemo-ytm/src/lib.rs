//! YouTube Music client: sign-in, stream resolver, SponsorBlock and timed lyrics.
//!
//! Access is unofficial and may breach YouTube's Terms of Service (see `coc.md`).
//! Stream resolution is anonymous: no cookies are ever passed to `yt-dlp`.

pub mod sponsorblock;
pub mod ytdlp;
