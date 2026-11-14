//! Tokenizer and attribute reading for the hand-written TTML scanner.

pub(super) enum Tok<'a> {
    Start {
        name: &'a str,
        attrs: &'a str,
        empty: bool,
    },
    End {
        name: &'a str,
    },
    Text {
        text: &'a str,
        raw: bool,
    },
}

/// Local part of a qualified name (`ttm:agent` gives `agent`).
pub(super) fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// The next token, or `None` at the end. Comments, declarations and processing instructions are
/// skipped.
pub(super) fn next_token<'a>(src: &'a str, pos: &mut usize) -> Option<Tok<'a>> {
    loop {
        let rest = src.get(*pos..)?;
        if rest.is_empty() {
            return None;
        }
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            *pos += end;
            return Some(Tok::Text {
                text: &rest[..end],
                raw: false,
            });
        }
        if let Some(body) = rest.strip_prefix("<!--") {
            *pos += match body.find("-->") {
                Some(i) => 4 + i + 3,
                None => rest.len(),
            };
            continue;
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let (text, used) = match body.find("]]>") {
                Some(i) => (&body[..i], 9 + i + 3),
                None => (body, rest.len()),
            };
            *pos += used;
            return Some(Tok::Text { text, raw: true });
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            *pos += rest.find('>').map_or(rest.len(), |i| i + 1);
            continue;
        }
        if let Some(body) = rest.strip_prefix("</") {
            let close = body.find('>');
            let name = &body[..close.unwrap_or(body.len())];
            *pos += 2 + close.map_or(body.len(), |i| i + 1);
            return Some(Tok::End { name: name.trim() });
        }
        // A start tag. Find its closing '>' outside quotes.
        let body = &rest[1..];
        let mut quote: Option<char> = None;
        let mut close = None;
        for (i, c) in body.char_indices() {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (None, '"' | '\'') => quote = Some(c),
                (None, '>') => {
                    close = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            // Unterminated tag: drop the rest.
            *pos += rest.len();
            return None;
        };
        *pos += 1 + close + 1;
        let inner = &body[..close];
        let (inner, empty) = match inner.strip_suffix('/') {
            Some(i) => (i, true),
            None => (inner, false),
        };
        let name_end = inner
            .find(|c: char| c.is_whitespace())
            .unwrap_or(inner.len());
        return Some(Tok::Start {
            name: &inner[..name_end],
            attrs: &inner[name_end..],
            empty,
        });
    }
}

/// Decode the five XML entities and numeric references. Unknown entities stay as written.
pub(super) fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let decoded = tail.find(';').filter(|&j| j <= 10).and_then(|j| {
            let name = &tail[1..j];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => name.strip_prefix('#').and_then(|n| {
                    let code = match n.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok()?,
                        None => n.parse::<u32>().ok()?,
                    };
                    char::from_u32(code)
                }),
            };
            c.map(|c| (c, j + 1))
        });
        if let Some((c, used)) = decoded {
            out.push(c);
            rest = &tail[used..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Attributes as `(local name, decoded value)`.
pub(super) fn parse_attrs(s: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim_start();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else { break };
        let name = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let body = &after[1..];
        let Some(end) = body.find(q) else { break };
        if !name.is_empty() {
            out.push((local(name), decode_entities(&body[..end])));
        }
        rest = body[end + 1..].trim_start();
    }
    out
}

pub(super) fn attr<'a>(attrs: &'a [(&str, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}
