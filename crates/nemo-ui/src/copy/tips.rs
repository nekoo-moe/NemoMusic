//! One quiet line on Home. Picked by day, so it does not change on every start.

use crate::i18n::t;
use crate::settings::Language;

/// What kind of line it is; the UI shows it as a small label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Useful usage tip.
    Tip,
    /// Keyboard shortcut hint.
    Shortcut,
    /// Facts about the application architecture and privacy.
    FunFact,
}

/// One line of text.
pub struct Tip {
    /// Category of the tip.
    pub kind: Kind,
    /// English text.
    pub en: &'static str,
    /// Vietnamese text.
    pub vi: &'static str,
}

impl Tip {
    /// Localized text according to the user's chosen language.
    #[must_use]
    pub fn localized(&self, lang: Language) -> &'static str {
        t(lang, self.en, self.vi)
    }
}

/// The list of tips, shortcuts, and fun facts.
pub const TIPS: &[Tip] = &[
    Tip {
        kind: Kind::Shortcut,
        en: "Press / or Ctrl+K to search from anywhere.",
        vi: "Nhấn / hoặc Ctrl+K để tìm kiếm từ bất kỳ đâu.",
    },
    Tip {
        kind: Kind::Shortcut,
        en: "Press [ and ] to nudge lyrics timing by 50 ms.",
        vi: "Nhấn [ và ] để tinh chỉnh khớp lời bài hát 50 ms.",
    },
    Tip {
        kind: Kind::Tip,
        en: "The badge in the player bar opens the signal path.",
        vi: "Huy hiệu chất lượng trên thanh phát nhạc sẽ mở đường truyền âm thanh.",
    },
    Tip {
        kind: Kind::FunFact,
        en: "NemoMusic has no server. Your sign-in and library stay on this computer.",
        vi: "NemoMusic không có máy chủ. Đăng nhập và thư viện nằm hoàn toàn trên máy bạn.",
    },
    Tip {
        kind: Kind::Tip,
        en: "Switch between YouTube Music and SoundCloud in Your accounts.",
        vi: "Chuyển đổi giữa YouTube Music và SoundCloud trong mục Tài khoản.",
    },
];

/// The line for `day` (days since 1970-01-01).
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "day % TIPS.len() is always strictly smaller than TIPS.len()"
)]
pub fn of_the_day(day: u64) -> &'static Tip {
    &TIPS[(day % TIPS.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_the_day_is_stable_for_same_day() {
        let t1 = of_the_day(19000);
        let t2 = of_the_day(19000);
        assert_eq!(t1.en, t2.en);
        assert_eq!(t1.vi, t2.vi);
        assert_eq!(t1.kind, t2.kind);
    }

    #[test]
    fn tips_list_is_not_empty_and_localized() {
        assert!(!TIPS.is_empty());
        for tip in TIPS {
            assert!(!tip.localized(Language::English).is_empty());
            assert!(!tip.localized(Language::Vietnamese).is_empty());
        }
    }
}
