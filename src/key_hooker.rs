use std::cell::RefCell;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, WH_KEYBOARD_LL,
};

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case, clippy::upper_case_acronyms)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: u32,
    pub scanCode: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

type EventCallback = Box<dyn Fn(&KBDLLHOOKSTRUCT) -> bool>;

thread_local! {
    static HOOK_STATE: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

struct HookState {
    hook_handle: HHOOK,
    on_event: EventCallback,
}

pub struct KeyHooker;

impl KeyHooker {
    pub fn install(on_event: EventCallback) -> Result<Self, windows::core::Error> {
        let already_installed =
            HOOK_STATE.with(|s| s.try_borrow().map(|state| state.is_some()).unwrap_or(true));
        if already_installed {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::ERROR_ALREADY_EXISTS.into(),
                "KeyHooker already installed on this thread",
            ));
        }

        let hmodule = unsafe { GetModuleHandleW(None)? };
        let hinstance = HINSTANCE(hmodule.0);

        let hook_handle =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hinstance, 0) }?;

        let installed = HOOK_STATE.with(|s| {
            let Ok(mut state) = s.try_borrow_mut() else {
                return false;
            };
            *state = Some(HookState {
                hook_handle,
                on_event,
            });
            true
        });

        if installed {
            Ok(KeyHooker)
        } else {
            unsafe {
                let _ = UnhookWindowsHookEx(hook_handle);
            }
            Err(windows::core::Error::from_win32())
        }
    }
}

impl Drop for KeyHooker {
    fn drop(&mut self) {
        HOOK_STATE.with(|s| {
            let Ok(mut slot) = s.try_borrow_mut() else {
                return;
            };
            if let Some(state) = slot.take() {
                unsafe {
                    let _ = UnhookWindowsHookEx(state.hook_handle);
                }
            }
        });
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 || lparam.0 == 0 {
        return call_next_hook(code, wparam, lparam);
    }

    let khs: &KBDLLHOOKSTRUCT = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

    let should_swallow = HOOK_STATE.with(|s| {
        let Ok(state) = s.try_borrow() else {
            return None;
        };
        state.as_ref().map(|s| (s.on_event)(khs))
    });

    if should_swallow == Some(true) {
        LRESULT(1)
    } else {
        call_next_hook(code, wparam, lparam)
    }
}

fn hook_handle() -> Option<HHOOK> {
    HOOK_STATE.with(|s| s.try_borrow().ok()?.as_ref().map(|state| state.hook_handle))
}

fn call_next_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match hook_handle() {
        Some(hhook) => unsafe { CallNextHookEx(hhook, code, wparam, lparam) },
        None => LRESULT(0),
    }
}

pub fn get_vkey(khs: &KBDLLHOOKSTRUCT) -> u8 {
    khs.vkCode as u8
}

pub fn is_key_up(khs: &KBDLLHOOKSTRUCT) -> bool {
    (khs.flags & 0x80) != 0
}
