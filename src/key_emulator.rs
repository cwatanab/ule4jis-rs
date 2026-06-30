use std::cell::RefCell;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, GetKeyboardLayout, MapVirtualKeyExW, SendInput, HKL, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC_EX, VIRTUAL_KEY, VK_CANCEL, VK_DELETE, VK_DIVIDE, VK_DOWN,
    VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_LSHIFT, VK_NEXT, VK_NUMLOCK, VK_PRIOR, VK_RCONTROL,
    VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_SHIFT, VK_SNAPSHOT, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::key_condition::KeyCondition;
use crate::key_hooker::{self, KeyHooker, KBDLLHOOKSTRUCT};

static NEXT_EMULATOR_ID: AtomicUsize = AtomicUsize::new(1);
const MAX_EMULATION_EVENTS: usize = 10;
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
    logical_lshift_released: bool,
    logical_rshift_released: bool,
    logical_lshift_pressed: bool,
    logical_rshift_pressed: bool,
    active_emulation_key: Option<u8>,
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
                logical_lshift_released: false,
                logical_rshift_released: false,
                logical_lshift_pressed: false,
                logical_rshift_pressed: false,
                active_emulation_key: None,
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

        // Reset logical adjustment flags when physical modifier keys are pressed/released
        if vkey == VK_LSHIFT.0 as u8 || vkey == VK_SHIFT.0 as u8 {
            inner.logical_lshift_released = false;
            inner.logical_lshift_pressed = false;
            inner.active_emulation_key = None;
        } else if vkey == VK_RSHIFT.0 as u8 {
            inner.logical_rshift_released = false;
            inner.logical_rshift_pressed = false;
            inner.active_emulation_key = None;
        }

        let cmp = inner.key_condition.cmp_value() as usize;
        let mapping = crate::strategy::US_ON_JIS_MAP[cmp];

        // Restore modifiers if another key is pressed/released while modifier adjustments are active
        let mut restoration_events = Vec::new();
        if inner.active_emulation_key.is_some_and(|k| k != vkey) {
            restoration_events = Self::restore_modifiers(inner);
        }

        if let Some(emulation) = mapping {
            let mut events = restoration_events;
            let key_condition = inner.key_condition;
            emulation.execute(up, &key_condition, |ev_vkey, ev_key_up| {
                events.push((ev_vkey, ev_key_up));
                Self::update_modifier_tracking(inner, ev_vkey, ev_key_up);
            });

            if inner.logical_lshift_released
                || inner.logical_rshift_released
                || inner.logical_lshift_pressed
                || inner.logical_rshift_pressed
            {
                inner.active_emulation_key = Some(vkey);
            } else {
                inner.active_emulation_key = None;
            }

            if !events.is_empty() {
                Self::send_events(&events, inner.emulator_id, inner.input_backend);
            }
            true
        } else {
            if !restoration_events.is_empty() {
                let mut events = restoration_events;
                events.push((vkey, up));
                Self::send_events(&events, inner.emulator_id, inner.input_backend);
                true
            } else {
                false
            }
        }
    }

    fn restore_modifiers(inner: &mut EmulatorInner) -> Vec<(u8, bool)> {
        let mut events = Vec::new();
        if inner.logical_lshift_released {
            events.push((VK_LSHIFT.0 as u8, false));
            inner.logical_lshift_released = false;
        }
        if inner.logical_rshift_released {
            events.push((VK_RSHIFT.0 as u8, false));
            inner.logical_rshift_released = false;
        }
        if inner.logical_lshift_pressed {
            events.push((VK_LSHIFT.0 as u8, true));
            inner.logical_lshift_pressed = false;
        }
        if inner.logical_rshift_pressed {
            events.push((VK_RSHIFT.0 as u8, true));
            inner.logical_rshift_pressed = false;
        }
        inner.active_emulation_key = None;
        events
    }

    fn update_modifier_tracking(inner: &mut EmulatorInner, vkey: u8, key_up: bool) {
        if vkey == VK_LSHIFT.0 as u8 || vkey == VK_SHIFT.0 as u8 {
            let physical_down = inner.key_condition.get_mod_key_state(VK_LSHIFT.0);
            if key_up {
                if physical_down {
                    inner.logical_lshift_released = true;
                    inner.logical_lshift_pressed = false;
                } else {
                    inner.logical_lshift_released = false;
                    inner.logical_lshift_pressed = false;
                }
            } else {
                if !physical_down {
                    inner.logical_lshift_pressed = true;
                    inner.logical_lshift_released = false;
                } else {
                    inner.logical_lshift_pressed = false;
                    inner.logical_lshift_released = false;
                }
            }
        } else if vkey == VK_RSHIFT.0 as u8 {
            let physical_down = inner.key_condition.get_mod_key_state(VK_RSHIFT.0);
            if key_up {
                if physical_down {
                    inner.logical_rshift_released = true;
                    inner.logical_rshift_pressed = false;
                } else {
                    inner.logical_rshift_released = false;
                    inner.logical_rshift_pressed = false;
                }
            } else {
                if !physical_down {
                    inner.logical_rshift_pressed = true;
                    inner.logical_rshift_released = false;
                } else {
                    inner.logical_rshift_pressed = false;
                    inner.logical_rshift_released = false;
                }
            }
        }
    }

    fn send_events(events: &[(u8, bool)], emulator_id: usize, input_backend: InputBackend) {
        match input_backend {
            InputBackend::SendInput => {
                Self::send_inputs(events, emulator_id);
            }
            InputBackend::KeybdEvent => {
                for &(vkey, key_up) in events {
                    Self::send_legacy_input(vkey, key_up, emulator_id);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_LSHIFT;

    #[test]
    fn test_modifier_tracking_and_restoration() {
        let mut inner = EmulatorInner {
            key_condition: KeyCondition::default(),
            emulator_id: 1,
            input_backend: InputBackend::SendInput,
            logical_lshift_released: false,
            logical_rshift_released: false,
            logical_lshift_pressed: false,
            logical_rshift_pressed: false,
            active_emulation_key: None,
        };

        // 1. Simulate physical shift held down
        inner
            .key_condition
            .change_key_state(VK_LSHIFT.0 as u8, false);

        // 2. Simulate mapping where LSHIFT was logically released (e.g. from ShiftRelease)
        KeyEmulator::update_modifier_tracking(&mut inner, VK_LSHIFT.0 as u8, true);
        assert!(inner.logical_lshift_released);
        assert!(!inner.logical_lshift_pressed);

        // 3. Verify restoration logic generates the correct event (press LSHIFT down) and clears state
        inner.active_emulation_key = Some(b'2');
        let events = KeyEmulator::restore_modifiers(&mut inner);
        assert_eq!(events, vec![(VK_LSHIFT.0 as u8, false)]);
        assert!(!inner.logical_lshift_released);
        assert!(inner.active_emulation_key.is_none());
    }
}
