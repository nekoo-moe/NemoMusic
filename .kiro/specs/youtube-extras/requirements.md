## Goal

NemoMusic reaches past the music catalog when the user asks. Search can cover all of YouTube, with videos and channels next to songs. Live stations and live streams play like any other source. Now Playing can show YouTube comments, and a signed-in user can post, reply and like. Dislike counts from Return YouTube Dislike can appear next to a video. Each part is opt-in and says what it contacts.

## User Stories

- As a listener, I want to search all of YouTube, so that I find live sets, talks and uploads that are not in the music catalog.
- As a listener, I want to play live streams and live radio stations, so that I can leave one running.
- As a listener, I want to read comments on the song, so that I can see what others say.
- As a signed-in user, I want to post, reply and like comments, so that I can join in without a browser.
- As a listener, I want to see the dislike count, so that I can judge a video before I play it.

## Acceptance Criteria

1. THE SYSTEM SHALL provide a search scope switch: "Music" (default) and "All of YouTube".
2. WHEN the scope is "All of YouTube", THE SYSTEM SHALL return videos and channels in addition to the current result groups, and label each result by kind.
3. WHEN the user plays a video result, THE SYSTEM SHALL play its audio. WHERE the `video-playback` feature is on, it MAY show the picture.
4. WHEN the user opens a channel result, THE SYSTEM SHALL show its public uploads and playlists.
5. WHEN the scope is "Music", THE SYSTEM SHALL behave as today.
6. WHEN a result is a live stream, THE SYSTEM SHALL label it "Live" and SHALL NOT show a duration or a seek bar that implies an end.
7. WHEN a live stream is playing, THE SYSTEM SHALL show the real codec and bitrate, or "Unknown", and SHALL NOT say lossless.
8. IF a live stream ends or drops, THEN THE SYSTEM SHALL say so, stop, and SHALL NOT restart it without the user. One retry on the user's action is allowed.
9. WHEN the user opens the live area, THE SYSTEM SHALL list the live stations and streams YouTube Music offers. IF none are available, THEN THE SYSTEM SHALL say "No live streams right now".
10. THE SYSTEM SHALL provide a setting "Show comments in Now Playing", off by default, with a plain note that it contacts YouTube and may show text from strangers.
11. WHEN comments are on, THE SYSTEM SHALL load them for the playing item only when the user opens the comments panel. It SHALL NOT load them in the background.
12. WHEN comments load, THE SYSTEM SHALL show the author name, text, like count and reply count, and a "Load more" action. Pages load only on user action.
13. IF comments are disabled for a video, THEN THE SYSTEM SHALL say "Comments are turned off".
14. WHEN the user is signed in, THE SYSTEM SHALL let them post a comment, reply, and like or un-like a comment.
15. WHEN the user is not signed in, THE SYSTEM SHALL show comments read-only and hide the post actions.
16. WHEN the user presses "Post", THE SYSTEM SHALL send the text once, show the result, and keep the text if it fails.
17. THE SYSTEM SHALL provide a setting "Show dislike counts", off by default. BEFORE it turns on, THE SYSTEM SHALL say that the video ID is sent to `returnyoutubedislikeapi.com`, which keeps its own data, and that the count is an estimate.
18. WHEN dislike counts are on, THE SYSTEM SHALL ask for the count only for the item the user opens, at most once per item per session, and SHALL cache it locally for at most a day.
19. WHEN the service answers 429, THEN THE SYSTEM SHALL stop asking for a while and show nothing instead of a wrong count.
20. IF the service is down or has no data (404), THEN THE SYSTEM SHALL show "Unknown", never zero.
21. WHEN a count is shown, THE SYSTEM SHALL label it "Estimated by Return YouTube Dislike" with a link to returnyoutubedislike.com.
22. WHEN the user signs out, THE SYSTEM SHALL delete cached comments, cached dislike counts and any account-specific data from these features.
23. THE SYSTEM SHALL keep every request human-paced and honor backoff signals.
24. WHEN a new host is used, THE SYSTEM SHALL list it in the README egress list in the same change: `returnyoutubedislikeapi.com`. `www.youtube.com` and `music.youtube.com` are already listed.
25. WHEN any panel from this spec is open, THE SYSTEM SHALL be operable by keyboard alone, with a visible focus.
26. WHERE the user has reduced motion on, THE SYSTEM SHALL show comment loading and the live marker without animation.
27. WHEN the user first switches the scope to "All of YouTube", THE SYSTEM SHALL show a one-line note that results come from outside the music catalog, and SHALL show it once.
28. WHEN a video result is queued, THE SYSTEM SHALL show it as a video item with its channel name, and the badge SHALL show the real codec and bitrate of its audio.

## Out of Scope

- Downloading or exporting video, comments or live streams.
- Moderating comments, reporting them, or reading a channel's community posts.
- Live chat of live streams.
- Uploading videos, and managing a channel.
- Showing ads or ad-supported features.
- Voting on dislikes through Return YouTube Dislike (sending the user's own vote). Read only.
- Extra charts from chart.simpmusic.org or any other external chart site (dropped by the owner, 2026-11-14). The built-in Charts stay as they are.

## Open Questions

Decided by the owner on 2026-11-14:

- The external chart site is dropped: its criteria and question are removed and it sits in Out of Scope.
- Return YouTube Dislike's terms are confirmed in the design (attribution link, rate limits). The innertube calls for comments (read, post, reply, like) and for searching all of YouTube are checked in the design.

Still open:

1. Account risk of posting comments through the web innertube client. The design explains the calls; the owner decides whether posting ships in the first version or after a trial period.
2. Whether `yt-dlp` today returns a live stream the decoder can play (the design needs a spike with a real live URL).

<!-- Sources:
- Repo: .kiro/steering/product.md, coc.md; crates/nemo-ytm/src/catalog.rs, feeds.rs, innertube.rs, stream.rs; crates/nemo-core/src/catalog.rs (SearchResults); docs/adr/0004-feature-wave-crates.md
- Return YouTube Dislike API terms, base URL, `/votes?videoId=`, rate limits: https://github.com/Anarios/return-youtube-dislike/blob/main/README.md
-->