use std::cell::RefCell;

use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;

use crate::i18n;
use crate::key_emulator::KeyEmulator;
use crate::tray::{self, TrayState};

thread_local! {
    pub static GLOBAL_APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

pub struct App {
    pub emulator: KeyEmulator,
    pub tray: TrayState,
}

impl App {
    pub fn new() -> Self {
        Self {
            emulator: KeyEmulator::new(),
            tray: TrayState::default(),
        }
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
        GLOBAL_APP.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(f))
    }

    pub fn install(self) -> bool {
        GLOBAL_APP.with(|cell| {
            let Ok(mut app) = cell.try_borrow_mut() else {
                return false;
            };
            if app.is_some() {
                return false;
            }
            *app = Some(self);
            true
        })
    }

    fn take_global() -> Option<Self> {
        GLOBAL_APP.with(|cell| cell.try_borrow_mut().ok()?.take())
    }

    pub fn shutdown_global() {
        let mut app = Self::take_global();
        if let Some(ref mut app) = app {
            app.emulator.stop();
            tray::cleanup(&mut app.tray);
        }
    }

    pub fn start_with_error(&mut self, error_key: &str) {
        match self.emulator.start() {
            Ok(()) => tray::update_tray_icon(&mut self.tray, true),
            Err(e) => i18n::show_error(error_key, &e.to_string()),
        }
    }

    fn set_emulator_running(&mut self, running: bool) {
        if running {
            self.start_with_error("error.start_failed");
        } else {
            self.emulator.stop();
            tray::update_tray_icon(&mut self.tray, false);
        }
    }
}

pub fn on_tray_toggle() {
    App::with_mut(|app| app.set_emulator_running(!app.emulator.is_started()));
}

pub fn on_tray_start() {
    App::with_mut(|app| app.set_emulator_running(true));
}

pub fn on_tray_stop() {
    App::with_mut(|app| app.set_emulator_running(false));
}

pub fn on_tray_show_window() {
    let Some((Some(hwnd), running, input_backend)) = App::with_mut(|app| {
        let running = app.emulator.is_started();
        let input_backend = app.emulator.input_backend();
        (app.tray.window, running, input_backend)
    }) else {
        return;
    };
    tray::show_context_menu(hwnd, running, input_backend);
}

pub fn on_tray_toggle_input_backend() {
    App::with_mut(|app| app.emulator.toggle_input_backend());
}

pub fn on_tray_exit() {
    App::shutdown_global();
    unsafe { PostQuitMessage(0) };
}
