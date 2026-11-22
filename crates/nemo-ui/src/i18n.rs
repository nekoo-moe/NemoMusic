//! Internationalisation (i18n) support for NemoMusic.
//!
//! Provides localized strings for English and Vietnamese (Tiếng Việt) across the application,
//! focusing on the First-time Setup wizard and Settings (About, Section navigation, etc.).

#![allow(missing_docs)]

use crate::copy;
use crate::settings::Language;
use crate::setup::SetupStep;

/// Helper to select string based on active language.
#[must_use]
pub const fn t(lang: Language, en: &'static str, vi: &'static str) -> &'static str {
    match lang {
        Language::English => en,
        Language::Vietnamese => vi,
    }
}

/// Setup step title (for stepper and headings).
#[must_use]
pub const fn step_title(step: SetupStep, lang: Language) -> &'static str {
    match (step, lang) {
        (SetupStep::Welcome, Language::Vietnamese) => "Chào mừng",
        (SetupStep::Welcome, Language::English) => "Welcome",
        (SetupStep::Look, Language::Vietnamese) => "Giao diện",
        (SetupStep::Look, Language::English) => "Look",
        (SetupStep::Notice, Language::Vietnamese) => "Quyền riêng tư & Rủi ro",
        (SetupStep::Notice, Language::English) => "Privacy and risk",
        (SetupStep::Sources, Language::Vietnamese) => "Nguồn nhạc",
        (SetupStep::Sources, Language::English) => "Sources",
        (SetupStep::Playback, Language::Vietnamese) => "Phát nhạc",
        (SetupStep::Playback, Language::English) => "Playback",
        (SetupStep::Done, Language::Vietnamese) => "Hoàn tất",
        (SetupStep::Done, Language::English) => "Done",
    }
}

/// Setup step counter line.
#[must_use]
pub fn step_place(step: SetupStep, lang: Language) -> String {
    let count = SetupStep::ALL.len();
    let idx = step.index() + 1;
    match lang {
        Language::Vietnamese => format!("Bước {idx} trên {count}"),
        Language::English => format!("Step {idx} of {count}"),
    }
}

/// Wording for the Welcome step.
pub struct WelcomeText {
    pub title: &'static str,
    pub headline: &'static str,
    pub local_title: &'static str,
    pub local_body: &'static str,
    pub time_title: &'static str,
    pub time_body: &'static str,
    pub language_label: &'static str,
}

#[must_use]
pub const fn welcome_text(lang: Language) -> WelcomeText {
    match lang {
        Language::Vietnamese => WelcomeText {
            title: "Chào mừng bạn đến với NemoMusic",
            headline: "NemoMusic phát nhạc từ YouTube Music và các tệp âm thanh trên máy tính này, hiển thị lời bài hát đồng bộ theo từng câu hát.",
            local_title: "Chạy hoàn toàn trên máy tính",
            local_body: "Đăng nhập, thư viện và lịch sử được lưu trữ hoàn toàn trên máy tính này. Chúng tôi không có bất kỳ máy chủ nào.",
            time_title: "Chỉ mất khoảng một phút",
            time_body: "Tiếp theo là vài tùy chọn: giao diện, nguồn nhạc và trải nghiệm phát nhạc. Bạn có thể bỏ qua bất cứ lúc nào và chỉnh sửa lại sau trong Cài đặt.",
            language_label: "Ngôn ngữ",
        },
        Language::English => WelcomeText {
            title: copy::setup::WELCOME_TITLE,
            headline: copy::setup::WELCOME[0],
            local_title: copy::setup::WELCOME_LOCAL_TITLE,
            local_body: copy::setup::WELCOME[1],
            time_title: copy::setup::WELCOME_TIME_TITLE,
            time_body: copy::setup::WELCOME[2],
            language_label: "Language",
        },
    }
}

/// Wording for the Look step.
pub struct LookText {
    pub title: &'static str,
    pub help: &'static str,
    pub match_song: &'static str,
    pub match_song_help: &'static str,
    pub breathing: &'static str,
    pub breathing_help: &'static str,
    pub reduce_motion: &'static str,
    pub surface: &'static str,
    pub accent: &'static str,
    pub corners: &'static str,
    pub now_playing: &'static str,
}

#[must_use]
pub const fn look_text(lang: Language) -> LookText {
    match lang {
        Language::Vietnamese => LookText {
            title: "Chọn giao diện",
            help: "Thay đổi áp dụng ngay khi bạn chọn. Bạn có thể thay đổi lại sau trong Cài đặt.",
            match_song: "Phối màu theo bài hát đang phát",
            match_song_help: "Toàn bộ cửa sổ đổi màu sắc hài hòa theo ảnh bìa của bài hát đang phát.",
            breathing: "Thở nhẹ nhàng",
            breathing_help: "Hiệu ứng chuyển động thở chậm rãi, mềm mại của màu nền khi đang phát nhạc.",
            reduce_motion: "Giảm chuyển động",
            surface: "Chế độ nền",
            accent: "Cường độ màu nhấn",
            corners: "Bo góc",
            now_playing: "Kiểu hiển thị Now Playing",
        },
        Language::English => LookText {
            title: copy::setup::LOOK_TITLE,
            help: copy::setup::LOOK_HELP,
            match_song: copy::SONG_COLOUR_LABEL,
            match_song_help: copy::SONG_COLOUR_HELPER,
            breathing: copy::HEARTBEAT_LABEL,
            breathing_help: copy::HEARTBEAT_HELPER,
            reduce_motion: "Reduce motion",
            surface: "Surface",
            accent: "Accent strength",
            corners: "Corners",
            now_playing: "Now Playing style",
        },
    }
}

/// Notice step text.
pub struct NoticeText {
    pub title: &'static str,
    pub paragraphs: [&'static str; 3],
    pub checkbox: &'static str,
    pub continue_btn: &'static str,
    pub not_now_btn: &'static str,
    pub accepted: &'static str,
    pub foot: &'static str,
}

#[must_use]
pub fn notice_text(lang: Language) -> NoticeText {
    match lang {
        Language::Vietnamese => NoticeText {
            title: "Trước khi bắt đầu",
            paragraphs: [
                "NemoMusic không được tạo ra, phê duyệt hay hỗ trợ bởi Google hoặc YouTube. Ứng dụng sử dụng YouTube Music qua các phương thức không chính thức, có thể vi phạm Điều khoản dịch vụ của YouTube. Google có thể giới hạn hoặc đình chỉ tài khoản bạn đăng nhập và quyền truy cập có thể dừng hoạt động bất kỳ lúc nào mà không báo trước. Hãy tự chịu rủi ro khi sử dụng và cân nhắc dùng tài khoản phụ.",
                "NemoMusic chạy hoàn toàn trên máy tính của bạn và không có máy chủ: thông tin đăng nhập và thư viện nhạc được lưu trữ an toàn ngay trên máy tính này. Đăng xuất sẽ xóa cả hai.",
                "Chặn quảng cáo và SponsorBlock có thể làm giảm thu nhập của nghệ sĩ; cả hai đều có thể tắt trong Cài đặt. Âm thanh YouTube Music là lossy.",
            ],
            checkbox: "Tôi đã đọc và hiểu thông báo này",
            continue_btn: "Tiếp tục",
            not_now_btn: "Để sau",
            accepted: "Bạn đã đồng ý với thông báo này.",
            foot: "Nhạc cục bộ của bạn luôn phát được mà không cần bước này. YouTube Music sẽ chờ đến khi bạn tiếp tục.",
        },
        Language::English => NoticeText {
            title: copy::setup::NOTICE_TITLE,
            paragraphs: copy::TOS_NOTICE,
            checkbox: copy::TOS_CHECKBOX,
            continue_btn: "Continue",
            not_now_btn: "Not now",
            accepted: copy::setup::NOTICE_ACCEPTED,
            foot: copy::setup::NOTICE_FOOT,
        },
    }
}

/// Sources step text.
pub struct SourcesText {
    pub title: &'static str,
    pub help: &'static str,
    pub yt_title: &'static str,
    pub yt_body: &'static str,
    pub yt_sign_in: &'static str,
    pub yt_no_account: &'static str,
    pub yt_using_without: &'static str,
    pub sc_title: &'static str,
    pub sc_body: &'static str,
    pub sc_sign_in: &'static str,
    pub local_title: &'static str,
    pub local_body: &'static str,
    pub local_btn: &'static str,
}

#[must_use]
pub const fn sources_text(lang: Language) -> SourcesText {
    match lang {
        Language::Vietnamese => SourcesText {
            title: "Nguồn nhạc của bạn",
            help: "Sử dụng bất kỳ nguồn nào bạn thích. Bạn có thể thay đổi sau trong Cài đặt.",
            yt_title: "YouTube Music",
            yt_body: "Đăng nhập để xem thư viện, bài hát yêu thích và gợi ý. Không cần tài khoản bạn vẫn có thể tìm kiếm và phát nhạc.",
            yt_sign_in: "Đăng nhập với Google",
            yt_no_account: "Dùng không cần tài khoản",
            yt_using_without: "Đang dùng YouTube Music không cần tài khoản.",
            sc_title: "SoundCloud",
            sc_body: "Đăng nhập để xem các bài hát đã thích, danh sách phát và lịch sử nghe nhạc từ SoundCloud.",
            sc_sign_in: "Đăng nhập với SoundCloud",
            local_title: "Nhạc trên máy tính",
            local_body: "Phát các tệp âm thanh trong các thư mục trên máy tính này, hỗ trợ đầy đủ các định dạng lossless.",
            local_btn: "Thêm thư mục",
        },
        Language::English => SourcesText {
            title: copy::setup::SOURCES_TITLE,
            help: copy::setup::SOURCES_HELP,
            yt_title: "YouTube Music",
            yt_body: copy::setup::YT_BODY,
            yt_sign_in: "Sign in with Google",
            yt_no_account: "Use without an account",
            yt_using_without: copy::setup::YT_WITHOUT_ACCOUNT,
            sc_title: copy::setup::SC_TITLE,
            sc_body: "Sign in to see your likes, playlists, artists and history from SoundCloud.",
            sc_sign_in: "Sign in with SoundCloud",
            local_title: "Local music",
            local_body: copy::setup::LOCAL_BODY,
            local_btn: "Add folders",
        },
    }
}

/// Playback step text.
pub struct PlaybackText {
    pub title: &'static str,
    pub help: &'static str,
    pub sponsorblock_label: &'static str,
    pub sponsorblock_help: &'static str,
    pub adblock_label: &'static str,
    pub adblock_help: &'static str,
}

#[must_use]
pub const fn playback_text(lang: Language) -> PlaybackText {
    match lang {
        Language::Vietnamese => PlaybackText {
            title: "Phát nhạc",
            help: "Cả hai tùy chọn đều có thể bật/tắt tại đây hoặc trong Cài đặt bất cứ lúc nào.",
            sponsorblock_label: "Bỏ qua các đoạn không phải âm nhạc trong video ca nhạc",
            sponsorblock_help: "Dữ liệu được đóng góp từ cộng đồng SponsorBlock",
            adblock_label: "Không yêu cầu hoặc phát quảng cáo",
            adblock_help: "Điều này có thể giới hạn tài khoản của bạn và làm giảm thu nhập của nghệ sĩ. Tính năng này không mở khóa chất lượng Premium.",
        },
        Language::English => PlaybackText {
            title: copy::setup::PLAYBACK_TITLE,
            help: copy::setup::PLAYBACK_HELP,
            sponsorblock_label: copy::SPONSORBLOCK_LABEL,
            sponsorblock_help: copy::SPONSORBLOCK_SOURCE,
            adblock_label: copy::ADBLOCK_LABEL,
            adblock_help: copy::ADBLOCK_HELPER,
        },
    }
}

/// Done step text.
pub struct DoneText {
    pub title: &'static str,
    pub more: &'static str,
    pub start_button: &'static str,
}

#[must_use]
pub const fn done_text(lang: Language) -> DoneText {
    match lang {
        Language::Vietnamese => DoneText {
            title: "Bạn đã sẵn sàng!",
            more: "Nhiều tùy chọn hơn trong Cài đặt: Last.fm, Discord, Cast và các tính năng AI sẽ giữ tắt cho đến khi bạn bật lên tại đó.",
            start_button: "Bắt đầu nghe nhạc",
        },
        Language::English => DoneText {
            title: copy::setup::DONE_TITLE,
            more: copy::setup::DONE_MORE,
            start_button: "Start listening",
        },
    }
}

/// Setup navigation button labels.
pub struct SetupNavText {
    pub skip: &'static str,
    pub back: &'static str,
    pub next: &'static str,
}

#[must_use]
pub const fn setup_nav(lang: Language) -> SetupNavText {
    match lang {
        Language::Vietnamese => SetupNavText {
            skip: "Bỏ qua thiết lập",
            back: "Quay lại",
            next: "Tiếp tục",
        },
        Language::English => SetupNavText {
            skip: "Skip setup",
            back: "Back",
            next: "Next",
        },
    }
}

/// Settings > About texts.
pub struct AboutText {
    pub language_title: &'static str,
    pub language_help: &'static str,
    pub updates_title: &'static str,
    pub updates_help: &'static str,
    pub version_label: &'static str,
    pub licenses_title: &'static str,
    pub licenses_body: &'static str,
    pub credits_title: &'static str,
    pub tos_title: &'static str,
    pub tos_body: &'static str,
    pub review_notice: &'static str,
    pub setup_title: &'static str,
    pub setup_help: &'static str,
    pub setup_button: &'static str,
}

#[must_use]
pub const fn about_text(lang: Language) -> AboutText {
    match lang {
        Language::Vietnamese => AboutText {
            language_title: "Ngôn ngữ",
            language_help: "Chọn ngôn ngữ hiển thị cho giao diện NemoMusic.",
            updates_title: "Kiểm tra bản cập nhật",
            updates_help: "Kiểm tra bản phát hành trên GitHub tối đa 1 lần/ngày. Chỉ gửi User-Agent: NemoMusic/<phiên bản>.",
            version_label: "Phiên bản",
            licenses_title: "Giấy phép",
            licenses_body: "NemoMusic được phát hành theo giấy phép GPL-3.0. Phần mềm mã nguồn mở, độc lập và phi thương mại.",
            credits_title: "Ghi nhận đóng góp",
            tos_title: "Điều khoản dịch vụ YouTube Music",
            tos_body: "NemoMusic sử dụng YouTube Music qua các phương thức không chính thức và không có sự liên kết với Google hay YouTube. Vui lòng tự chịu rủi ro khi sử dụng tài khoản của bạn.",
            review_notice: "Xem lại thông báo khi khởi chạy lần đầu",
            setup_title: "Thiết lập ban đầu",
            setup_help: "Chạy lại các bước thiết lập lần đầu. Những gì bạn đã cấu hình sẽ không bị mất và bạn vẫn được giữ đăng nhập.",
            setup_button: "Chạy lại thiết lập",
        },
        Language::English => AboutText {
            language_title: "Language",
            language_help: "Choose the display language for the interface.",
            updates_title: "Check for updates",
            updates_help: "Check GitHub Releases at most once a day. Only sends User-Agent: NemoMusic/<version>.",
            version_label: "Version",
            licenses_title: "Licences",
            licenses_body: copy::ABOUT_LICENSE,
            credits_title: "Credits",
            tos_title: "YouTube Music",
            tos_body: copy::ABOUT_TOS,
            review_notice: "Review the first-run notice",
            setup_title: "Setup",
            setup_help: copy::setup::RERUN_HELP,
            setup_button: "Run setup again",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_labels_and_methods() {
        assert_eq!(Language::English.label(), "English");
        assert_eq!(Language::Vietnamese.label(), "Tiếng Việt");
        assert!(!Language::English.is_vietnamese());
        assert!(Language::Vietnamese.is_vietnamese());
        assert_eq!(Language::ALL.len(), 2);
    }

    #[test]
    fn step_titles_and_places() {
        for step in SetupStep::ALL {
            assert!(!step_title(step, Language::English).is_empty());
            assert!(!step_title(step, Language::Vietnamese).is_empty());
            assert!(step_place(step, Language::English).contains("Step"));
            assert!(step_place(step, Language::Vietnamese).contains("Bước"));
        }
    }

    #[test]
    fn text_bundles_are_complete() {
        for lang in Language::ALL {
            let welcome = welcome_text(lang);
            assert!(!welcome.title.is_empty() && !welcome.headline.is_empty());
            let look = look_text(lang);
            assert!(!look.title.is_empty() && !look.help.is_empty());
            let notice = notice_text(lang);
            assert!(!notice.title.is_empty() && notice.paragraphs.len() == 3);
            let sources = sources_text(lang);
            assert!(!sources.title.is_empty() && !sources.yt_title.is_empty());
            let playback = playback_text(lang);
            assert!(!playback.title.is_empty() && !playback.sponsorblock_label.is_empty());
            let done = done_text(lang);
            assert!(!done.title.is_empty() && !done.start_button.is_empty());
            let about = about_text(lang);
            assert!(!about.language_title.is_empty() && !about.setup_button.is_empty());
        }
    }
}
