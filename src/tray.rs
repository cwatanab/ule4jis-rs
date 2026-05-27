// System tray icon + context menu management.

use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DestroyWindow, GetCursorPos, PostQuitMessage, RegisterClassExW,
    TrackPopupMenu, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HICON, IMAGE_FLAGS, MENU_ITEM_FLAGS,
    MF_GRAYED, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN, TPM_RIGHTALIGN, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSEXW,
};

// ── embedded icons ─────────────────────────────────────────────────

const ICON_ENABLED: &[u8] = include_bytes!("../assets/app_enabled.ico");
const ICON_DISABLED: &[u8] = include_bytes!("../assets/app_disabled.ico");

// ── icon cache (thread-local because HICON is !Send+!Sync) ────────

thread_local! {
    /// Cached HICON handles — parsed once per thread on first use.
    static CACHED_ICON_ENABLED: RefCell<Option<HICON>> = const { RefCell::new(None) };
    static CACHED_ICON_DISABLED: RefCell<Option<HICON>> = const { RefCell::new(None) };
}

/// Retrieve a parsed icon from the embedded resource, or from the cache
/// if already parsed.
fn get_cached_icon(
    data: &'static [u8],
    cache: &'static std::thread::LocalKey<RefCell<Option<HICON>>>,
) -> Option<HICON> {
    cache.with(|c| {
        let mut opt = c.borrow_mut();
        if opt.is_none() {
            *opt = ico_to_hicon(data);
        }
        opt.filter(|h| !h.is_invalid())
    })
}

/// Parse a `.ico` byte slice and create an HICON.
///
/// The returned `HICON` must be destroyed with `DestroyIcon` when no longer
/// needed.  Callers that pass the icon to `Shell_NotifyIconW` can let the
/// shell own the lifetime.
fn ico_to_hicon(data: &[u8]) -> Option<HICON> {
    // ICO header: 6 bytes (reserved, type, count)
    if data.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;

    // Pick the best entry: largest pixel area, preferring 32x32
    let mut best_offset = 0u32;
    let mut best_size = 0u32;
    let mut best_area = 0u32;
    let entry_base = 6;

    for i in 0..count {
        let off = entry_base + i * 16;
        if off + 16 > data.len() {
            return None;
        }
        let w = data[off] as u32;
        let h = data[off + 1] as u32;
        // 0 in ICO means 256
        let w = if w == 0 { 256 } else { w };
        let h = if h == 0 { 256 } else { h };
        let area = w * h;
        let img_size =
            u32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
        let img_offset = u32::from_le_bytes([
            data[off + 12],
            data[off + 13],
            data[off + 14],
            data[off + 15],
        ]);

        // Prefer 32x32, then larger area, then first match
        let is_32 = w == 32 && h == 32;
        let better = best_area == 0 || (best_area != 1024 && (is_32 || area > best_area));

        if better {
            best_area = area;
            best_offset = img_offset;
            best_size = img_size;
        }
    }

    if best_size == 0 || best_offset as usize + best_size as usize > data.len() {
        return None;
    }

    let image_data = &data[best_offset as usize..(best_offset + best_size) as usize];

    // CreateIconFromResourceEx expects the raw icon image data (PNG or DIB).
    // SAFETY: `image_data` is a slice of the embedded ICO resource, valid for
    // the lifetime of the process. The function creates a new GDI icon object.
    let hicon = unsafe {
        CreateIconFromResourceEx(
            image_data,
            windows::Win32::Foundation::BOOL::from(true), // fIcon
            0x00030000,                                   // dwVersion
            0,                                            // cxDesired
            0,                                            // cyDesired
            IMAGE_FLAGS(0),                               // flags = LR_DEFAULTCOLOR
        )
    };

    match hicon {
        Ok(h) if !h.is_invalid() => Some(h),
        _ => None,
    }
}

// ── constants ───────────────────────────────────────────────────────

pub const WM_TRAY_CALLBACK: u32 = 0x8000 + 1;

pub const ID_TRAY_START: usize = 1001;
pub const ID_TRAY_STOP: usize = 1002;
pub const ID_TRAY_EXIT: usize = 1004;

pub type TrayCallback = Rc<dyn Fn(TrayCommand)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    Start,
    Stop,
    Exit,
    ShowWindow,
    Toggle,
}

thread_local! {
    static TRAY_CALLBACK: RefCell<Option<TrayCallback>> = const { RefCell::new(None) };
    static TRAY_NID: RefCell<Option<NOTIFYICONDATAW>> = const { RefCell::new(None) };
    static TRAY_WINDOW: RefCell<Option<HWND>> = const { RefCell::new(None) };
}

pub fn get_tray_hwnd() -> Option<HWND> {
    TRAY_WINDOW.with(|w| *w.borrow())
}

fn dispatch_tray_command(command: TrayCommand) {
    let callback = TRAY_CALLBACK.with(|c| c.borrow().as_ref().cloned());
    if let Some(cb) = callback {
        cb(command);
    }
}

pub fn init_tray(callback: TrayCallback) -> Result<HWND, windows::core::Error> {
    // SAFETY: Passing None returns the HMODULE of the current process image,
    // which is always valid.
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

    // SAFETY: `wc` has a valid cbSize, a valid window procedure, hInstance,
    // and a valid class name. All pointers are valid for this call.
    let atom = unsafe { RegisterClassExW(&wc) };
    if atom == 0 {
        return Err(windows::core::Error::from_win32());
    }

    // SAFETY: class_name was registered above, hinstance is valid, and all
    // pointers / handles are valid for the duration of this call.
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

    TRAY_CALLBACK.with(|c| *c.borrow_mut() = Some(callback));
    TRAY_WINDOW.with(|w| *w.borrow_mut() = Some(hwnd));

    Ok(hwnd)
}

pub fn add_tray_icon(hwnd: HWND, tooltip: &str) -> Result<(), windows::core::Error> {
    let icon = get_cached_icon(ICON_ENABLED, &CACHED_ICON_ENABLED)
        .ok_or_else(windows::core::Error::from_win32)?;

    let tip_wide: Vec<u16> = tooltip.encode_utf16().chain(Some(0)).collect();

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

    // SAFETY: nid is a valid NOTIFYICONDATAW with correct cbSize, hWnd is a
    // valid message-only window, and hIcon is a valid icon handle.
    let success = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    if !success.as_bool() {
        return Err(windows::core::Error::from_win32());
    }

    TRAY_NID.with(|n| *n.borrow_mut() = Some(nid));

    Ok(())
}

pub fn show_context_menu(hwnd: HWND, emulator_running: bool) {
    let mut point = Default::default();
    // SAFETY: `&mut point` is a valid pointer to a POINT struct.
    unsafe {
        let _ = GetCursorPos(&mut point);
    };

    // SAFETY: All Win32 menu APIs below are called with valid handles and
    // null-terminated wide string pointers. The menu is destroyed before
    // this block exits.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };

        let start_flags = if emulator_running {
            MENU_ITEM_FLAGS((MF_STRING | MF_GRAYED).0)
        } else {
            MENU_ITEM_FLAGS(MF_STRING.0)
        };
        {
            let txt = crate::i18n::tr_wide("tray.start");
            if AppendMenuW(
                menu,
                start_flags,
                ID_TRAY_START,
                windows::core::PCWSTR(txt.as_ptr()),
            )
            .is_err()
            {
                DestroyMenu(menu).ok();
                return;
            }
        }

        let stop_flags = if emulator_running {
            MENU_ITEM_FLAGS(MF_STRING.0)
        } else {
            MENU_ITEM_FLAGS((MF_STRING | MF_GRAYED).0)
        };
        {
            let txt = crate::i18n::tr_wide("tray.stop");
            if AppendMenuW(
                menu,
                stop_flags,
                ID_TRAY_STOP,
                windows::core::PCWSTR(txt.as_ptr()),
            )
            .is_err()
            {
                DestroyMenu(menu).ok();
                return;
            }
        }

        if AppendMenuW(
            menu,
            MENU_ITEM_FLAGS(MF_SEPARATOR.0),
            0,
            windows::core::w!(""),
        )
        .is_err()
        {
            DestroyMenu(menu).ok();
            return;
        }
        {
            let txt = crate::i18n::tr_wide("tray.exit");
            if AppendMenuW(
                menu,
                MENU_ITEM_FLAGS(MF_STRING.0),
                ID_TRAY_EXIT,
                windows::core::PCWSTR(txt.as_ptr()),
            )
            .is_err()
            {
                DestroyMenu(menu).ok();
                return;
            }
        }

        let _ = TrackPopupMenu(
            menu,
            TPM_BOTTOMALIGN | TPM_RIGHTALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        );

        // `menu` was created by CreatePopupMenu above and has not been
        // destroyed yet. TrackPopupMenu does not take ownership.
        DestroyMenu(menu).ok();
    }
}

pub fn remove_tray_icon() {
    TRAY_NID.with(|n| {
        if let Some(nid) = n.borrow().as_ref() {
            // SAFETY: nid is the NOTIFYICONDATAW previously added via NIM_ADD.
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, nid) };
        }
        *n.borrow_mut() = None;
    });
}

/// Switch the tray icon between active and inactive states.
///
/// Icons are cached after the first call — subsequent toggles reuse
/// the same handles without re-parsing the embedded ICO data.
pub fn update_tray_icon(active: bool) {
    TRAY_NID.with(|n| {
        if let Some(nid) = n.borrow_mut().as_mut() {
            let (data, cache) = if active {
                (ICON_ENABLED, &CACHED_ICON_ENABLED)
            } else {
                (ICON_DISABLED, &CACHED_ICON_DISABLED)
            };
            if let Some(icon) = get_cached_icon(data, cache) {
                nid.hIcon = icon;
                nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
                // SAFETY: nid is a valid NOTIFYICONDATAW previously added
                // via NIM_ADD.
                unsafe {
                    let _ = Shell_NotifyIconW(NIM_MODIFY, nid);
                }
            }
        }
    });
}

pub fn cleanup() {
    remove_tray_icon();

    // Destroy cached icon handles if they were ever created.
    CACHED_ICON_ENABLED.with(|c| {
        if let Some(h) = c.borrow_mut().take() {
            if !h.is_invalid() {
                // SAFETY: h was created by ico_to_hicon and cached. It is no
                // longer needed after the tray icon is removed.
                unsafe {
                    DestroyIcon(h).ok();
                }
            }
        }
    });
    CACHED_ICON_DISABLED.with(|c| {
        if let Some(h) = c.borrow_mut().take() {
            if !h.is_invalid() {
                unsafe {
                    DestroyIcon(h).ok();
                }
            }
        }
    });

    TRAY_CALLBACK.with(|c| *c.borrow_mut() = None);
    TRAY_WINDOW.with(|w| {
        if let Some(hwnd) = w.borrow().as_ref() {
            // SAFETY: hwnd is the valid message-only window created in init_tray.
            let _ = unsafe { DestroyWindow(*hwnd) };
        }
        *w.borrow_mut() = None;
    });
}

/// Message-only window procedure for tray icon notifications.
///
/// # Safety
///
/// Called by Windows on the thread that owns the message queue.  `lparam`
/// and `wparam` are guaranteed by the OS to be valid for the lifetime
/// of this call.
unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            match lparam.0 as u32 {
                WM_LBUTTONUP => {
                    dispatch_tray_command(TrayCommand::Toggle);
                }
                WM_RBUTTONUP | WM_LBUTTONDBLCLK => {
                    dispatch_tray_command(TrayCommand::ShowWindow);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = wparam.0 & 0xFFFF;
            let command = match cmd {
                ID_TRAY_START => TrayCommand::Start,
                ID_TRAY_STOP => TrayCommand::Stop,
                ID_TRAY_EXIT => TrayCommand::Exit,
                _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
            };
            dispatch_tray_command(command);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
