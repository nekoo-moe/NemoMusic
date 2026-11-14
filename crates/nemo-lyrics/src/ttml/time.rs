//! TTML time expressions to milliseconds.

/// Largest time value accepted (a day), in milliseconds.
const MAX_TIME_MS: u64 = 86_400_000;

/// `"12"`, `"12.5"` to milliseconds, where the number counts `unit_ms` milliseconds.
fn decimal_to_ms(s: &str, unit_ms: u64) -> Option<u64> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    if (int.is_empty() && frac.is_empty())
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole = if int.is_empty() {
        0
    } else {
        int.parse::<u64>().ok()?
    };
    let mut milli = 0u64;
    for (i, b) in frac.bytes().take(3).enumerate() {
        milli += u64::from(b - b'0') * [100, 10, 1][i];
    }
    let ms = whole
        .checked_mul(unit_ms)?
        .checked_add(milli * unit_ms / 1000)?;
    (ms <= MAX_TIME_MS).then_some(ms)
}

/// Parse a TTML time expression into milliseconds.
#[must_use]
pub fn parse_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("ms") {
        return decimal_to_ms(n, 1);
    }
    if let Some(n) = s.strip_suffix('s') {
        return decimal_to_ms(n, 1000);
    }
    if let Some(n) = s.strip_suffix('m') {
        return decimal_to_ms(n, 60_000);
    }
    if let Some(n) = s.strip_suffix('h') {
        return decimal_to_ms(n, 3_600_000);
    }
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [sec] => ("0", "0", *sec),
        [m, sec] => ("0", *m, *sec),
        [h, m, sec] => (*h, *m, *sec),
        _ => return None,
    };
    let int = |t: &str| {
        (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse::<u64>().ok())
            .flatten()
    };
    let total = int(h)?
        .checked_mul(3_600_000)?
        .checked_add(int(m)?.checked_mul(60_000)?)?
        .checked_add(decimal_to_ms(sec, 1000)?)?;
    (total <= MAX_TIME_MS).then_some(total)
}
