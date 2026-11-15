//! Comment types for a video (youtube-extras). Reading, posting and liking go through
//! [`CommentSource`](crate::services::CommentSource).

/// Id of one comment or reply. Treat it as untrusted text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommentId(pub String);

/// Opaque token for the next page of comments or the replies of one comment. Untrusted text:
/// pass it back to the source unchanged, never parse or build it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommentCursor(pub String);

/// One comment or reply. All text is untrusted and is shown as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// Id for liking and replying.
    pub id: CommentId,
    /// Display name of the author.
    pub author: String,
    /// The comment text.
    pub text: String,
    /// When it was posted, as the site writes it (`2 weeks ago`). `None` when not given.
    pub posted: Option<String>,
    /// Like count, if the site gives one.
    pub likes: Option<u64>,
    /// How many replies it has.
    pub reply_count: u32,
    /// The signed-in listener liked it.
    pub liked: bool,
    /// Cursor for its replies, or `None` when it has none to load.
    pub replies: Option<CommentCursor>,
    /// The signed-in listener wrote it.
    pub mine: bool,
}

/// One page of comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentPage {
    /// The comments of this page, in the site's order.
    pub comments: Vec<Comment>,
    /// Cursor for the next page, or `None` on the last page.
    pub next: Option<CommentCursor>,
    /// Whether the listener may post here (signed in and comments on).
    pub can_post: bool,
}

/// Reading or writing comments failed. Texts are fixed and never hold a response body.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommentError {
    /// The video has comments turned off.
    #[error("comments are turned off")]
    Disabled,
    /// The action needs a signed-in account.
    #[error("sign in to do this")]
    SignInNeeded,
    /// The service asked to slow down (HTTP 429). Nothing is retried automatically.
    #[error("too many requests; try again later")]
    RateLimited,
    /// The service refused the post (for example a spam filter). The text stays with the
    /// listener.
    #[error("the comment was not accepted")]
    Rejected,
    /// The network request failed.
    #[error("network error: {0}")]
    Network(String),
    /// The answer could not be parsed (the layout changed).
    #[error("could not parse comments: {0}")]
    Parse(String),
    /// Comments are not available (not supported, or the source is off).
    #[error("not available: {0}")]
    Unavailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_error_texts_hold_no_content() {
        for (error, text) in [
            (CommentError::Disabled, "comments are turned off"),
            (CommentError::SignInNeeded, "sign in to do this"),
            (
                CommentError::RateLimited,
                "too many requests; try again later",
            ),
            (CommentError::Rejected, "the comment was not accepted"),
        ] {
            assert_eq!(error.to_string(), text);
        }
    }

    #[test]
    fn a_cursor_is_kept_as_given() {
        let cursor = CommentCursor("EkMSC...==".to_owned());
        let page = CommentPage {
            comments: Vec::new(),
            next: Some(cursor.clone()),
            can_post: false,
        };
        assert_eq!(page.next, Some(cursor));
    }
}
