// Key emulator — receives hook events and dispatches to the emulation map.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_CANCEL, VK_DELETE,
    VK_DIVIDE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT, VK_NUMLOCK, VK_PRIOR,
    VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_SNAPSHOT, VK_UP,
};

use crate::emulation::KeyEmulatorApi;
use crate::key_condition::KeyCondition;
use crate::key_hooker::{self, KeyHooker, KBDLLHOOKSTRUCT};
use crate::strategy::EmulationMap;

static NEXT_EMULATOR_ID: AtomicUsize = AtomicUsize::new(1);

struct EmulatorInner {
    emulation_map: EmulationMap,
    key_condition: KeyCondition,
    emulator_id: usize,
}

pub struct KeyEmulator {
    inner: Rc<RefCell<EmulatorInner>>,
    hooker: Option<KeyHooker>,
}

impl KeyEmulator {
    pub fn new(strategy: &dyn crate::strategy::EmulationStrategy) -> Self {
        let mut map = EmulationMap::new();
        strategy.build_map(&mut map);

        let emulator_id = NEXT_EMULATOR_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            inner: Rc::new(RefCell::new(EmulatorInner {
                emulation_map: map,
                key_condition: KeyCondition::new(),
                emulator_id,
            })),
            hooker: None,
        }
    }

    pub fn start(&mut self) -> Result<(), windows::core::Error> {
        if self.hooker.is_some() {
            return Ok(());
        }

        let inner = Rc::clone(&self.inner);
        let hooker = KeyHooker::install(Box::new(move |khs| {
            match inner.try_borrow_mut() {
                Ok(mut inner) => Self::process_hook_event(&mut inner, khs),
                Err(_) => false, // re-entrant call from keybd_event; pass through
            }
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

    fn process_hook_event(inner: &mut EmulatorInner, khs: &KBDLLHOOKSTRUCT) -> bool {
        if khs.dwExtraInfo == inner.emulator_id {
            return false;
        }

        let vkey = key_hooker::get_vkey(khs);
        let up = key_hooker::is_key_up(khs);

        inner.key_condition.change_key_state(vkey, up);

        // Avoid cloning KeyCondition: look up by packed cmp_value (u16)
        // directly, then pass shared references to the stored condition
        // for decorators that need modifier state.
        let cmp = inner.key_condition.cmp_value();
        if let Some(emulation) = inner.emulation_map.get(&cmp) {
            if up {
                emulation.execute_up(&*inner, &inner.key_condition);
            } else {
                emulation.execute_down(&*inner, &inner.key_condition);
            }
            return true;
        }

        false
    }

    /// Precomputed O(1) lookup table for extended keys (over 256 entries).
    const EXTENDED_KEY_TABLE: [bool; 256] = {
        let mut t = [false; 256];
        t[VK_RCONTROL.0 as usize] = true;
        t[VK_RMENU.0 as usize] = true;
        t[VK_RSHIFT.0 as usize] = true;
        t[VK_INSERT.0 as usize] = true;
        t[VK_DELETE.0 as usize] = true;
        t[VK_HOME.0 as usize] = true;
        t[VK_END.0 as usize] = true;
        t[VK_PRIOR.0 as usize] = true;
        t[VK_NEXT.0 as usize] = true;
        t[VK_UP.0 as usize] = true;
        t[VK_DOWN.0 as usize] = true;
        t[VK_RIGHT.0 as usize] = true;
        t[VK_LEFT.0 as usize] = true;
        t[VK_NUMLOCK.0 as usize] = true;
        t[VK_CANCEL.0 as usize] = true;
        t[VK_SNAPSHOT.0 as usize] = true;
        t[VK_DIVIDE.0 as usize] = true;
        t
    };

    fn is_extended_key(vkey: u8) -> bool {
        Self::EXTENDED_KEY_TABLE[vkey as usize]
    }
}

impl KeyEmulatorApi for EmulatorInner {
    fn emulate_key(&self, vkey: u8, up: bool) {
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        if KeyEmulator::is_extended_key(vkey) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        // SAFETY: vkey is a valid virtual key code, flags are valid
        // KEYBD_EVENT_FLAGS, and emulator_id is used to tag injected events
        // so the hook can filter them out.
        unsafe {
            keybd_event(vkey, 0, flags, self.emulator_id);
        }
    }
}
