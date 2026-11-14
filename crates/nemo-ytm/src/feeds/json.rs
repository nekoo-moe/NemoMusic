//! JSON helpers shared by the feed readers.

use nemo_core::track::Thumbnail;
use serde_json::Value;

use crate::catalog::map_thumbnails;

/// First value stored under `key` anywhere inside `value` (depth-first).
pub(crate) fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

/// Text of the first run (or `simpleText`), trimmed; `None` when empty.
pub(crate) fn text_of(value: &Value) -> Option<String> {
    let text = value
        .pointer("/runs/0/text")
        .or_else(|| value.get("simpleText"))?
        .as_str()?
        .trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// All runs joined, trimmed; `None` when empty.
pub(super) fn joined_text(value: &Value) -> Option<String> {
    let runs = value.get("runs")?.as_array()?;
    let text: String = runs
        .iter()
        .filter_map(|r| r.get("text").and_then(Value::as_str))
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

pub(super) fn runs_of(value: &Value) -> &[Value] {
    value
        .get("runs")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// All sizes of the first `thumbnails` list under `value`.
pub(crate) fn thumbnails_of(value: &Value) -> Vec<Thumbnail> {
    let list: Vec<(String, u64, u64)> = find_key(value, "thumbnails")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|t| {
                    Some((
                        t.get("url")?.as_str()?.to_owned(),
                        t.get("width").and_then(Value::as_u64).unwrap_or(0),
                        t.get("height").and_then(Value::as_u64).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    map_thumbnails(&list)
}

/// The cover of an item: `thumbnailRenderer` (cards) or `thumbnail` (rows). Never the small
/// channel picture in `thumbnailCornerOverlay`.
pub(super) fn item_thumbnails(item: &Value) -> Vec<Thumbnail> {
    item.get("thumbnailRenderer")
        .or_else(|| item.get("thumbnail"))
        .map(thumbnails_of)
        .unwrap_or_default()
}
