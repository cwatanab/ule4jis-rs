#![windows_subsystem = "windows"]

// Ule4Jis — US keyboard → JIS layout emulator (Rust rewrite)

mod decorator;
mod emulation;
mod i18n;
mod key_condition;
mod key_emulator;
mod key_hooker;
mod strategy;
mod tray;

use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, PostQuitMessage, MB_ICONERROR, MB_OK, MSG,
};

use key_emulator::KeyEmulator;
use strategy::USonJISStrategy;

/// Show an error message box to the user.  Because the binary uses
/// `#![windows_subsystem = "windows"]`, stderr is discarded — MessageBox
/// is the only way to surface errors.
fn show_error(title_key: &str, msg: &str) {
    let title = i18n::tr(title_key);
    let title_wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let msg_wide: Vec<u16> = msg.encode_utf16().chain(Some(0)).collect();
    // SAFETY: Both wide strings are NUL-terminated and valid for the
    // duration of this call. HWND::default() means no owner window.
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR::from_raw(msg_wide.as_ptr()),
            PCWSTR::from_raw(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialise locale from ULE4JIS_LANG env var (ja / en, default ja).
    i18n::init();

    // Prevent multiple instances via a named mutex.
    // SAFETY: Passing None for lpMutexAttributes uses default security.
    // The wide string literal is valid for the process lifetime.
    let mutex = unsafe {
        CreateMutexW(
            None,
            true,
            windows::core::w!("Ule4Jis_SingleInstance_Mutex"),
        )
    };

    if mutex.is_ok() {
        // SAFETY: GetLastError is always safe to call.  If the mutex
        // already existed, we exit silently.
        let err = unsafe { windows::Win32::Foundation::GetLastError() };
        if err == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
            return Ok(());
        }
    }

    // Build the emulator
    let strategy = USonJISStrategy;
    let emulator = Rc::new(RefCell::new(KeyEmulator::new(&strategy)));

    // Clone for the tray callback
    let emulator_for_tray = Rc::clone(&emulator);

    // Initialize tray icon
    let hwnd = tray::init_tray(Rc::new(move |cmd| {
        handle_tray_command(&emulator_for_tray, cmd);
    }))?;

    tray::add_tray_icon(hwnd, i18n::tr("tray.tooltip"))?;

    // Start the emulator by default
    {
        let mut emu = emulator.borrow_mut();
        match emu.start() {
            Ok(()) => tray::update_tray_icon(true),
            Err(e) => show_error("error.hook_failed", &e.to_string()),
        }
    }

    // Message loop
    // SAFETY: mem::zeroed() zero-initialises the MSG struct; all fields
    // are valid for GetMessageW to fill.
    let mut msg: MSG = unsafe { mem::zeroed() };
    loop {
        // SAFETY: &mut msg is a valid pointer to MSG.  Passing None for
        // hWnd and 0 for wMsgFilterMin/Max retrieves all messages.
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 <= 0 {
            break;
        }
        // SAFETY: msg was populated by GetMessageW above and contains
        // a valid message for this thread.
        unsafe {
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    let mut emu = emulator.borrow_mut();
    emu.stop();
    tray::cleanup();

    Ok(())
}

/// Handle commands from the tray context menu.
///
/// Uses `try_borrow_mut` / `try_borrow` instead of `borrow_mut` / `borrow`
/// so re-entrant messages do not panic. Menu display copies the running
/// state before entering `TrackPopupMenu`.
fn handle_tray_command(emulator: &RefCell<KeyEmulator>, cmd: tray::TrayCommand) {
    match cmd {
        tray::TrayCommand::Start => {
            let Ok(mut emu) = emulator.try_borrow_mut() else {
                return;
            };
            match emu.start() {
                Ok(()) => tray::update_tray_icon(true),
                Err(e) => show_error("error.start_failed", &e.to_string()),
            }
        }
        tray::TrayCommand::Stop => {
            let Ok(mut emu) = emulator.try_borrow_mut() else {
                return;
            };
            emu.stop();
            tray::update_tray_icon(false);
        }
        tray::TrayCommand::Exit => {
            tray::cleanup();
            // SAFETY: PostQuitMessage is always safe; 0 as exit code
            // is the standard convention for "clean exit".
            unsafe { PostQuitMessage(0) };
        }
        tray::TrayCommand::Toggle => {
            let Ok(mut emu) = emulator.try_borrow_mut() else {
                return;
            };
            if emu.is_started() {
                emu.stop();
                tray::update_tray_icon(false);
            } else {
                match emu.start() {
                    Ok(()) => tray::update_tray_icon(true),
                    Err(e) => show_error("error.start_failed", &e.to_string()),
                }
            }
        }
        tray::TrayCommand::ShowWindow => {
            let running = {
                let Ok(emu) = emulator.try_borrow() else {
                    return;
                };
                emu.is_started()
            };
            let hwnd = tray::get_tray_hwnd().unwrap_or_default();
            tray::show_context_menu(hwnd, running);
        }
    }
}
