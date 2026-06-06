use std::cell::RefCell;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, GetKeyboardLayout, MapVirtualKeyExW, SendInput, HKL, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC_EX, VIRTUAL_KEY, VK_CANCEL, VK_DELETE, VK_DIVIDE, VK_DOWN,
    VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT, VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RIGHT,
    VK_RMENU, VK_RSHIFT, VK_SNAPSHOT, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::emulation::Emulation;
use crate::key_condition::KeyCondition;
use crate::key_hooker::{self, KeyHooker, KBDLLHOOKSTRUCT};

static NEXT_EMULATOR_ID: AtomicUsize = AtomicUsize::new(1);
const MAX_EMULATION_EVENTS: usize = 5;
#[rustfmt::skip]
const EXTENDED_KEYS: [u16; 15] = [VK_RCONTROL.0, VK_RMENU.0, VK_INSERT.0, VK_DELETE.0, VK_HOME.0, VK_END.0, VK_PRIOR.0, VK_NEXT.0, VK_UP.0, VK_DOWN.0, VK_RIGHT.0, VK_LEFT.0, VK_NUMLOCK.0, VK_CANCEL.0, VK_SNAPSHOT.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputBackend {
    SendInput,
    KeybdEvent,
}

impl InputBackend {
    fn toggled(self) -> Self {
        match self {
            Self::SendInput => Self::KeybdEvent,
            Self::KeybdEvent => Self::SendInput,
        }
    }
}

struct EmulatorInner {
    key_condition: KeyCondition,
    emulator_id: usize,
    input_backend: InputBackend,
}

pub struct KeyEmulator {
    inner: Rc<RefCell<EmulatorInner>>,
    hooker: Option<KeyHooker>,
}

impl KeyEmulator {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(EmulatorInner {
                key_condition: KeyCondition::default(),
                emulator_id: NEXT_EMULATOR_ID.fetch_add(1, Ordering::Relaxed),
                input_backend: InputBackend::SendInput,
            })),
            hooker: None,
        }
    }

    pub fn start(&mut self) -> Result<(), windows::core::Error> {
        if self.hooker.is_some() {
            return Ok(());
        }

        let inner = Rc::clone(&self.inner);
        let hooker = KeyHooker::install(Box::new(move |khs| match inner.try_borrow_mut() {
            Ok(mut inner) => Self::process_hook_event(&mut inner, khs),
            Err(_) => false,
        }))?;

        self.hooker = Some(hooker);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.hooker = None;
    }

    pub fn is_started(&self) -> bool {
        self.hooker.is_some()
    }

    pub fn input_backend(&self) -> InputBackend {
        self.inner
            .try_borrow()
            .map_or(InputBackend::SendInput, |inner| inner.input_backend)
    }

    pub fn toggle_input_backend(&mut self) -> InputBackend {
        let Ok(mut inner) = self.inner.try_borrow_mut() else {
            return InputBackend::SendInput;
        };
        inner.input_backend = inner.input_backend.toggled();
        inner.input_backend
    }

    fn process_hook_event(inner: &mut EmulatorInner, khs: &KBDLLHOOKSTRUCT) -> bool {
        if khs.dwExtraInfo == inner.emulator_id {
            return false;
        }

        let vkey = key_hooker::get_vkey(khs);
        let up = key_hooker::is_key_up(khs);

        inner.key_condition.change_key_state(vkey, up);

        let cmp = inner.key_condition.cmp_value() as usize;
        if let Some(emulation) = crate::strategy::US_ON_JIS_MAP[cmp] {
            return Self::send_emulation(
                emulation,
                up,
                &inner.key_condition,
                inner.emulator_id,
                inner.input_backend,
            );
        }

        false
    }

    fn send_emulation(
        emulation: Emulation,
        up: bool,
        key_condition: &KeyCondition,
        emulator_id: usize,
        input_backend: InputBackend,
    ) -> bool {
        match input_backend {
            InputBackend::SendInput => {
                let mut events = [(0, false); MAX_EMULATION_EVENTS];
                let mut event_count = 0;
                emulation.execute(up, key_condition, |vkey, key_up| {
                    debug_assert!(event_count < MAX_EMULATION_EVENTS);
                    if event_count < MAX_EMULATION_EVENTS {
                        events[event_count] = (vkey, key_up);
                        event_count += 1;
                    }
                });
                event_count == 0 || Self::send_inputs(&events[..event_count], emulator_id)
            }
            InputBackend::KeybdEvent => {
                emulation.execute(up, key_condition, |vkey, key_up| {
                    Self::send_legacy_input(vkey, key_up, emulator_id);
                });
                true
            }
        }
    }

    fn send_inputs(events: &[(u8, bool)], emulator_id: usize) -> bool {
        let layout = Self::foreground_keyboard_layout();
        let mut inputs = [INPUT::default(); MAX_EMULATION_EVENTS];

        for (input, &(vkey, key_up)) in inputs.iter_mut().zip(events) {
            *input = Self::input_for_event(vkey, key_up, emulator_id, layout);
        }

        let sent = unsafe { SendInput(&inputs[..events.len()], mem::size_of::<INPUT>() as i32) };
        sent as usize == events.len()
    }

    fn send_legacy_input(vkey: u8, key_up: bool, emulator_id: usize) {
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        if Self::is_legacy_extended_key(vkey) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        unsafe {
            keybd_event(vkey, 0, flags, emulator_id);
        }
    }

    fn input_for_event(vkey: u8, key_up: bool, emulator_id: usize, layout: HKL) -> INPUT {
        let (scan_code, scan_is_extended) = Self::map_vkey_to_scan(vkey, layout);

        let mut flags = KEYBD_EVENT_FLAGS(0);
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        if scan_code != 0 {
            flags |= KEYEVENTF_SCANCODE;
        }
        if scan_is_extended || Self::is_send_input_extended_key(vkey) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: if scan_code == 0 {
                        VIRTUAL_KEY(vkey as u16)
                    } else {
                        VIRTUAL_KEY(0)
                    },
                    wScan: scan_code,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: emulator_id,
                },
            },
        }
    }

    fn foreground_keyboard_layout() -> HKL {
        unsafe {
            let hwnd = GetForegroundWindow();
            let thread_id = GetWindowThreadProcessId(hwnd, None);
            GetKeyboardLayout(thread_id)
        }
    }

    fn map_vkey_to_scan(vkey: u8, layout: HKL) -> (u16, bool) {
        let mapped = unsafe { MapVirtualKeyExW(vkey as u32, MAPVK_VK_TO_VSC_EX, layout) };
        ((mapped & 0xFF) as u16, (mapped & 0xFF00) != 0)
    }

    const fn extended_key_table(include_rshift: bool) -> [bool; 256] {
        let mut t = [false; 256];
        let mut i = 0;
        while i < EXTENDED_KEYS.len() {
            t[EXTENDED_KEYS[i] as usize] = true;
            i += 1;
        }
        if include_rshift {
            t[VK_RSHIFT.0 as usize] = true;
        }
        t[VK_DIVIDE.0 as usize] = true;
        t
    }

    const SEND_INPUT_EXTENDED_KEY_TABLE: [bool; 256] = Self::extended_key_table(false);
    const LEGACY_EXTENDED_KEY_TABLE: [bool; 256] = Self::extended_key_table(true);

    fn is_send_input_extended_key(vkey: u8) -> bool {
        Self::SEND_INPUT_EXTENDED_KEY_TABLE[vkey as usize]
    }

    fn is_legacy_extended_key(vkey: u8) -> bool {
        Self::LEGACY_EXTENDED_KEY_TABLE[vkey as usize]
    }
}
