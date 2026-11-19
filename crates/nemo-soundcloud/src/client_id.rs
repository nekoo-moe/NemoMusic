//! Finding the web app's `client_id` (ADR 0005, Decision 2).
//!
//! The soundcloud.com home page lists its scripts; one of them holds `client_id:"<32 letters or
//! digits>"`. yt-dlp's extractor reads the same place. Both functions are pure; the fetching and
//! the one-shot refresh live in `client.rs`. The id is never logged and never stored in a file.

/// Longest list of scripts that are fetched while looking for the id.
pub const MAX_SCRIPTS: usize = 12;

/// Length of a client id.
const ID_LEN: usize = 32;

/// The `src` of every `<script ... src="...">` tag whose address starts with `prefix`, in page
/// order. The caller reads them from last to first, as the id sits in one of the last scripts.
#[must_use]
pub fn script_urls(html: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<script") {
        rest = &rest[at + "<script".len()..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        if let Some(src) = attribute(tag, "src")
            && src.starts_with(prefix)
            && !found.iter().any(|f| f == src)
        {
            found.push(src.to_owned());
        }
        rest = &rest[end..];
    }
    found
}

/// The value of `name="..."` inside one tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut from = 0;
    while let Some(at) = tag[from..].find(&key) {
        let start = from + at;
        // The match must start a word, so `data-src="` does not count as `src="`.
        let boundary = start == 0 || tag.as_bytes()[start - 1].is_ascii_whitespace();
        let value = &tag[start + key.len()..];
        if boundary {
            return value.split('"').next();
        }
        from = start + key.len();
    }
    None
}

/// The first `client_id:"<32 letters or digits>"` of a script (whitespace around the colon is
/// allowed). `None` when the script holds none.
#[must_use]
pub fn find_client_id(script: &str) -> Option<String> {
    let mut rest = script;
    while let Some(at) = rest.find("client_id") {
        rest = &rest[at + "client_id".len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(':') else {
            continue;
        };
        let Some(after) = after.trim_start().strip_prefix('"') else {
            continue;
        };
        let id: String = after
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if id.len() == ID_LEN && after[ID_LEN..].starts_with('"') {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // A made-up id; not a real one.
    const ID: &str = "AbCdEfGh0123456789AbCdEfGh012345";

    #[test]
    fn the_id_is_found_with_and_without_spaces() {
        assert_eq!(
            find_client_id(&format!("a,client_id:\"{ID}\",b")).as_deref(),
            Some(ID)
        );
        assert_eq!(
            find_client_id(&format!("client_id : \"{ID}\"")).as_deref(),
            Some(ID)
        );
    }

    #[test]
    fn a_script_without_an_id_gives_none() {
        assert_eq!(find_client_id("var x = 1; client_id"), None);
        assert_eq!(find_client_id("client_id:\"short\""), None);
        assert_eq!(
            find_client_id(&format!("client_id:\"{ID}x\"")),
            None,
            "33 characters"
        );
        assert_eq!(
            find_client_id(&format!("client_id:{ID}")),
            None,
            "no quotes"
        );
        assert_eq!(find_client_id(""), None);
    }

    #[test]
    fn the_first_valid_id_wins_and_a_bad_mention_is_skipped() {
        let other = "ZyXwVuTs9876543210ZyXwVuTs987654";
        let script =
            format!("client_id:\"nope\" client_id_x:1 client_id:\"{ID}\" client_id:\"{other}\"");
        assert_eq!(find_client_id(&script).as_deref(), Some(ID));
    }

    #[test]
    fn scripts_are_listed_in_page_order_on_the_prefix_only() {
        let html = r#"<html><script src="https://dwt.soundcloud.com/tags.js"></script>
            <script crossorigin src="https://a-v2.sndcdn.com/assets/59-aa.js"></script>
            <script>inline()</script>
            <script data-src="https://a-v2.sndcdn.com/assets/no.js" src="https://a-v2.sndcdn.com/assets/0-bb.js" defer></script>
            <script crossorigin src="https://a-v2.sndcdn.com/assets/59-aa.js"></script>
            <script src="https://evil.example/https://a-v2.sndcdn.com/x.js"></script>"#;
        assert_eq!(
            script_urls(html, "https://a-v2.sndcdn.com/"),
            [
                "https://a-v2.sndcdn.com/assets/59-aa.js",
                "https://a-v2.sndcdn.com/assets/0-bb.js"
            ]
        );
    }

    #[test]
    fn a_page_without_scripts_gives_nothing() {
        assert!(script_urls("<html></html>", "https://a-v2.sndcdn.com/").is_empty());
        assert!(
            script_urls(
                "<script src=\"https://a-v2.sndcdn.com/x.js\"",
                "https://a-v2.sndcdn.com/"
            )
            .is_empty()
        );
    }
}
