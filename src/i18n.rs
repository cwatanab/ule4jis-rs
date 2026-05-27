// Lightweight i18n: Japanese / English via compile-time map + global locale.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    Ja = 0,
    En = 1,
}

static CURRENT_LANG: AtomicU8 = AtomicU8::new(Lang::Ja as u8);

/// Initialise the global locale from `ULE4JIS_LANG`.
/// Recognised values: `"ja"` (default), `"en"`.
pub fn init() {
    let lang = match std::env::var("ULE4JIS_LANG").as_deref() {
        Ok("en") | Ok("EN") | Ok("En") => Lang::En,
        _ => Lang::Ja,
    };
    CURRENT_LANG.store(lang as u8, Ordering::Relaxed);
}

/// Set locale at runtime (e.g. from a tray menu entry).
#[allow(dead_code)]
pub fn set_lang(lang: Lang) {
    CURRENT_LANG.store(lang as u8, Ordering::Relaxed);
}

/// Get the current locale.
pub fn get_lang() -> Lang {
    match CURRENT_LANG.load(Ordering::Relaxed) {
        0 => Lang::Ja,
        _ => Lang::En,
    }
}

/// Look up a translation key.
///
/// Uses a compile-time-optimised `match` instead of a linear scan.
/// Returns the English fallback when the key is unknown.
pub fn tr(key: &str) -> &'static str {
    match (key, get_lang()) {
        ("tray.tooltip", Lang::Ja) => "Ule4Jis — US→JIS エミュレータ",
        ("tray.tooltip", Lang::En) => "Ule4Jis — US→JIS Emulator",
        ("tray.start", Lang::Ja) => "開始",
        ("tray.start", Lang::En) => "Start",
        ("tray.stop", Lang::Ja) => "停止",
        ("tray.stop", Lang::En) => "Stop",
        ("tray.exit", Lang::Ja) => "終了",
        ("tray.exit", Lang::En) => "Exit",
        ("error.hook_failed", Lang::Ja) => "キーボードフックの開始に失敗しました",
        ("error.hook_failed", Lang::En) => "Failed to start keyboard hook",
        ("error.start_failed", Lang::Ja) => "開始に失敗しました",
        ("error.start_failed", Lang::En) => "Failed to start",
        _ => "???",
    }
}

// ── helpers ──────────────────────────────────────────────────────────

/// Convert a `&str` to a NUL-terminated UTF-16 `Vec<u16>` suitable for
/// Win32 wide-string APIs.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Get a translated string and convert to a NUL-terminated wide vector.
pub fn tr_wide(key: &str) -> Vec<u16> {
    to_wide(tr(key))
}
