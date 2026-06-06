use std::mem;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, PostQuitMessage, RegisterClassExW, TrackPopupMenu, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HMENU, MENU_ITEM_FLAGS, MF_GRAYED, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN,
    TPM_RIGHTALIGN, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK,
    WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSEXW,
};

use crate::key_emulator::InputBackend;

pub const WM_TRAY_CALLBACK: u32 = 0x8000 + 1;

pub const ID_TRAY_START: usize = 1001;
pub const ID_TRAY_STOP: usize = 1002;
pub const ID_TRAY_TOGGLE_INPUT_BACKEND: usize = 1003;
pub const ID_TRAY_EXIT: usize = 1004;

#[derive(Default)]
pub struct TrayState {
    pub nid: Option<NOTIFYICONDATAW>,
    pub window: Option<HWND>,
}

struct Menu(HMENU);

impl Menu {
    fn new() -> Result<Self, windows::core::Error> {
        unsafe { CreatePopupMenu().map(Self) }
    }

    fn append(&self, flags: MENU_ITEM_FLAGS, id: usize, text_key: &str) -> Result<(), ()> {
        let wide = crate::i18n::tr_wide(text_key);
        unsafe { AppendMenuW(self.0, flags, id, PCWSTR(wide.as_ptr())) }.map_err(|_| ())
    }

    fn item(&self, id: usize, text_key: &str, enabled: bool) -> Result<(), ()> {
        let flags = if enabled {
            MENU_ITEM_FLAGS(MF_STRING.0)
        } else {
            MENU_ITEM_FLAGS((MF_STRING | MF_GRAYED).0)
        };
        self.append(flags, id, text_key)
    }

    fn separator(&self) -> Result<(), ()> {
        unsafe { AppendMenuW(self.0, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, PCWSTR::null()) }
            .map_err(|_| ())
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        unsafe {
            DestroyMenu(self.0).ok();
        }
    }
}

pub fn init_tray() -> Result<HWND, windows::core::Error> {
    let hmodule = unsafe { GetModuleHandleW(None)? };
    let hinstance = HINSTANCE(hmodule.0);

    let class_name = windows::core::w!("Ule4JisTrayWindow");
    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(tray_window_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };

    let atom = unsafe { RegisterClassExW(&wc) };
    if atom == 0 {
        return Err(windows::core::Error::from_win32());
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            windows::core::w!("Ule4Jis"),
            WINDOW_STYLE::default(),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            hinstance,
            None,
        )
    }?;

    if hwnd.is_invalid() {
        return Err(windows::core::Error::from_win32());
    }

    Ok(hwnd)
}

pub fn add_tray_icon(
    state: &mut TrayState,
    hwnd: HWND,
    tooltip: &str,
) -> Result<(), windows::core::Error> {
    let icon = crate::icon::get_icon(true).ok_or_else(windows::core::Error::from_win32)?;

    let tip_wide = crate::i18n::to_wide(tooltip);

    let mut nid = NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: icon,
        ..Default::default()
    };

    let max_len = nid.szTip.len().min(tip_wide.len());
    nid.szTip[..max_len].copy_from_slice(&tip_wide[..max_len]);

    let success = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    if !success.as_bool() {
        return Err(windows::core::Error::from_win32());
    }

    state.nid = Some(nid);
    Ok(())
}

pub fn update_tray_icon(state: &mut TrayState, active: bool) {
    if let Some(nid) = state.nid.as_mut() {
        let icon = crate::icon::get_icon(active);
        if let Some(icon) = icon {
            nid.hIcon = icon;
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, nid);
            }
        }
    }
}

pub fn remove_tray_icon(state: &mut TrayState) {
    if let Some(nid) = state.nid.take() {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}

pub fn show_context_menu(hwnd: HWND, emulator_running: bool, input_backend: InputBackend) {
    let mut point = Default::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
    };

    let Ok(menu) = Menu::new() else {
        return;
    };

    if populate_context_menu(&menu, emulator_running, input_backend).is_err() {
        return;
    }

    unsafe {
        let _ = TrackPopupMenu(
            menu.0,
            TPM_BOTTOMALIGN | TPM_RIGHTALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        );
    }
}

fn populate_context_menu(
    menu: &Menu,
    emulator_running: bool,
    input_backend: InputBackend,
) -> Result<(), ()> {
    menu.item(ID_TRAY_START, "tray.start", !emulator_running)?;
    menu.item(ID_TRAY_STOP, "tray.stop", emulator_running)?;
    menu.separator()?;
    menu.item(
        ID_TRAY_TOGGLE_INPUT_BACKEND,
        match input_backend {
            InputBackend::SendInput => "tray.input_backend.sendinput",
            InputBackend::KeybdEvent => "tray.input_backend.keybd_event",
        },
        true,
    )?;
    menu.separator()?;
    menu.item(ID_TRAY_EXIT, "tray.exit", true)
}

pub fn cleanup(state: &mut TrayState) {
    remove_tray_icon(state);
    crate::icon::cleanup_icons();

    if let Some(hwnd) = state.window.take() {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            match lparam.0 as u32 {
                WM_LBUTTONUP => crate::app::on_tray_toggle(),
                WM_RBUTTONUP | WM_LBUTTONDBLCLK => crate::app::on_tray_show_window(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = wparam.0 & 0xFFFF;
            match cmd {
                ID_TRAY_START => crate::app::on_tray_start(),
                ID_TRAY_STOP => crate::app::on_tray_stop(),
                ID_TRAY_TOGGLE_INPUT_BACKEND => crate::app::on_tray_toggle_input_backend(),
                ID_TRAY_EXIT => crate::app::on_tray_exit(),
                _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
            };
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
