use std::sync::atomic::{AtomicU8, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    Ja = 0,
    En = 1,
}

static CURRENT_LANG: AtomicU8 = AtomicU8::new(Lang::Ja as u8);

pub fn init() {
    let lang = match std::env::var("ULE4JIS_LANG") {
        Ok(v) if v.eq_ignore_ascii_case("en") => Lang::En,
        _ => Lang::Ja,
    };
    CURRENT_LANG.store(lang as u8, Ordering::Relaxed);
}

#[allow(dead_code)]
pub fn set_lang(lang: Lang) {
    CURRENT_LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn get_lang() -> Lang {
    match CURRENT_LANG.load(Ordering::Relaxed) {
        0 => Lang::Ja,
        _ => Lang::En,
    }
}

pub fn tr(key: &str) -> &'static str {
    let text = match key {
        "tray.tooltip" => ["Ule4Jis — US→JIS エミュレータ", "Ule4Jis — US→JIS Emulator"],
        "tray.start" => ["開始", "Start"],
        "tray.stop" => ["停止", "Stop"],
        "tray.input_backend.sendinput" => ["入力方式: SendInput", "Input: SendInput"],
        "tray.input_backend.keybd_event" => ["入力方式: keybd_event", "Input: keybd_event"],
        "tray.exit" => ["終了", "Exit"],
        "error.hook_failed" => [
            "キーボードフックの開始に失敗しました",
            "Failed to start keyboard hook",
        ],
        "error.start_failed" => ["開始に失敗しました", "Failed to start"],
        _ => return "???",
    };
    text[get_lang() as usize]
}

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn tr_wide(key: &str) -> PCWSTR {
    let lang = get_lang();
    match key {
        "tray.tooltip" => match lang {
            Lang::Ja => windows::core::w!("Ule4Jis — US→JIS エミュレータ"),
            Lang::En => windows::core::w!("Ule4Jis — US→JIS Emulator"),
        },
        "tray.start" => match lang {
            Lang::Ja => windows::core::w!("開始"),
            Lang::En => windows::core::w!("Start"),
        },
        "tray.stop" => match lang {
            Lang::Ja => windows::core::w!("停止"),
            Lang::En => windows::core::w!("Stop"),
        },
        "tray.input_backend.sendinput" => match lang {
            Lang::Ja => windows::core::w!("入力方式: SendInput"),
            Lang::En => windows::core::w!("Input: SendInput"),
        },
        "tray.input_backend.keybd_event" => match lang {
            Lang::Ja => windows::core::w!("入力方式: keybd_event"),
            Lang::En => windows::core::w!("Input: keybd_event"),
        },
        "tray.exit" => match lang {
            Lang::Ja => windows::core::w!("終了"),
            Lang::En => windows::core::w!("Exit"),
        },
        "error.hook_failed" => match lang {
            Lang::Ja => windows::core::w!("キーボードフックの開始に失敗しました"),
            Lang::En => windows::core::w!("Failed to start keyboard hook"),
        },
        "error.start_failed" => match lang {
            Lang::Ja => windows::core::w!("開始に失敗しました"),
            Lang::En => windows::core::w!("Failed to start"),
        },
        _ => windows::core::w!("???"),
    }
}

pub(crate) fn show_error(title_key: &str, msg: &str) {
    let title_wide = to_wide(tr(title_key));
    let msg_wide = to_wide(msg);
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR::from_raw(msg_wide.as_ptr()),
            PCWSTR::from_raw(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}
