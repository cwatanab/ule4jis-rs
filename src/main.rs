#![windows_subsystem = "windows"]

mod app;
mod emulation;
mod i18n;
mod icon;
mod key_condition;
mod key_emulator;
mod key_hooker;
mod strategy;
mod tray;

use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG};

use app::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    i18n::init();

    let mutex = unsafe {
        CreateMutexW(
            None,
            true,
            windows::core::w!("Ule4Jis_SingleInstance_Mutex"),
        )
    };

    if mutex.is_ok() {
        let err = unsafe { GetLastError() };
        if err == ERROR_ALREADY_EXISTS {
            return Ok(());
        }
    }

    let mut app = App::new();

    let hwnd = tray::init_tray()?;
    app.tray.window = Some(hwnd);

    if !app.install() {
        return Ok(());
    }

    if let Some(Err(e)) =
        App::with_mut(|app| tray::add_tray_icon(&mut app.tray, hwnd, i18n::tr("tray.tooltip")))
    {
        return Err(e.into());
    }

    App::with_mut(|app| app.start_with_error("error.hook_failed"));

    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 <= 0 {
            break;
        }
        unsafe {
            DispatchMessageW(&msg);
        }
    }

    App::shutdown_global();

    Ok(())
}
