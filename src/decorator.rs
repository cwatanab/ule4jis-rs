// Decorator wrappers for Emulation objects.
// Mirrors the C++ ShiftPressDecorator, ShiftReleaseDecorator, PressAndReleaseDecorator.

use crate::emulation::{Emulation, KeyEmulatorApi};
use crate::key_condition::KeyCondition;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LSHIFT, VK_RSHIFT};

// --- ShiftPressDecorator ---
pub struct ShiftPressDecorator {
    inner: Box<dyn Emulation>,
}

impl ShiftPressDecorator {
    pub fn new(inner: Box<dyn Emulation>) -> Self {
        Self { inner }
    }
}

impl Emulation for ShiftPressDecorator {
    fn execute_down(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition) {
        emulator.emulate_key(VK_LSHIFT.0 as u8, false);
        self.inner.execute_down(emulator, cond);
        emulator.emulate_key(VK_LSHIFT.0 as u8, true);
    }

    fn execute_up(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition) {
        self.inner.execute_up(emulator, cond);
    }
}

// --- ShiftReleaseDecorator ---
pub struct ShiftReleaseDecorator {
    inner: Box<dyn Emulation>,
}

impl ShiftReleaseDecorator {
    pub fn new(inner: Box<dyn Emulation>) -> Self {
        Self { inner }
    }
}

impl Emulation for ShiftReleaseDecorator {
    fn execute_down(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition) {
        let lshift = cond.get_mod_key_state(VK_LSHIFT.0);
        let rshift = cond.get_mod_key_state(VK_RSHIFT.0);

        if lshift {
            emulator.emulate_key(VK_LSHIFT.0 as u8, true);
        }
        if rshift {
            emulator.emulate_key(VK_RSHIFT.0 as u8, true);
        }

        self.inner.execute_down(emulator, cond);

        if lshift {
            emulator.emulate_key(VK_LSHIFT.0 as u8, false);
        }
        if rshift {
            emulator.emulate_key(VK_RSHIFT.0 as u8, false);
        }
    }

    fn execute_up(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition) {
        self.inner.execute_up(emulator, cond);
    }
}

// --- PressAndReleaseDecorator ---
pub struct PressAndReleaseDecorator {
    inner: Box<dyn Emulation>,
}

impl PressAndReleaseDecorator {
    pub fn new(inner: Box<dyn Emulation>) -> Self {
        Self { inner }
    }
}

impl Emulation for PressAndReleaseDecorator {
    fn execute_down(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition) {
        self.inner.execute_down(emulator, cond);
        self.inner.execute_up(emulator, cond);
    }

    fn execute_up(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition) {
        self.inner.execute_down(emulator, cond);
        self.inner.execute_up(emulator, cond);
    }
}
