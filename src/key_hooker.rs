// Low-level keyboard hook via SetWindowsHookExW(WH_KEYBOARD_LL).

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
        if HOOK_STATE.with(|s| s.borrow().is_some()) {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::ERROR_ALREADY_EXISTS.into(),
                "KeyHooker already installed on this thread",
            ));
        }

        // SAFETY: Passing None queries the current module's HMODULE, which is
        // always valid. The returned handle is valid for the lifetime of the
        // process.
        let hmodule = unsafe { GetModuleHandleW(None)? };
        let hinstance = HINSTANCE(hmodule.0);

        // SAFETY: hinstance is valid, hook_proc is a valid `extern "system"`
        // function, and we unregister the hook in Drop.
        let hook_handle =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hinstance, 0) }?;

        HOOK_STATE.with(|s| {
            *s.borrow_mut() = Some(HookState {
                hook_handle,
                on_event,
            });
        });

        Ok(KeyHooker)
    }
}

impl Drop for KeyHooker {
    fn drop(&mut self) {
        HOOK_STATE.with(|s| {
            if let Some(state) = s.borrow_mut().take() {
                // SAFETY: The hook handle was created by SetWindowsHookExW
                // during install and has not been previously unregistered.
                unsafe {
                    let _ = UnhookWindowsHookEx(state.hook_handle);
                }
            }
        });
    }
}

/// Low-level keyboard hook callback registered with `SetWindowsHookExW`.
///
/// # Safety
///
/// This function is called by Windows on the thread that installed the hook.
/// `lparam` must point to a valid `KBDLLHOOKSTRUCT`. We trust the OS to
/// uphold this contract.
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        let handle = HOOK_STATE.with(|s| s.borrow().as_ref().map(|state| state.hook_handle));
        if let Some(hhook) = handle {
            // SAFETY: `hhook` was returned by SetWindowsHookExW and has not
            // been unregistered. `code` < 0 so we must call CallNextHookEx.
            return CallNextHookEx(hhook, code, wparam, lparam);
        }
        return LRESULT(0);
    }

    // SAFETY: Windows guarantees that lparam points to a valid
    // KBDLLHOOKSTRUCT for the lifetime of this callback.
    let khs: &KBDLLHOOKSTRUCT = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

    let (should_swallow, hook_handle) = HOOK_STATE.with(|s| {
        let state = s.borrow();
        let cb = state.as_ref().map(|s| (s.on_event)(khs));
        let hk = state.as_ref().map(|s| s.hook_handle);
        (cb, hk)
    });

    if should_swallow == Some(true) {
        LRESULT(1)
    } else if let Some(hhook) = hook_handle {
        // SAFETY: hhook is valid per the same reasoning as above.
        CallNextHookEx(hhook, code, wparam, lparam)
    } else {
        LRESULT(0)
    }
}

pub fn get_vkey(khs: &KBDLLHOOKSTRUCT) -> u8 {
    khs.vkCode as u8
}

pub fn is_key_up(khs: &KBDLLHOOKSTRUCT) -> bool {
    (khs.flags & 0x80) != 0
}
