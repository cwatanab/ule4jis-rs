// Emulation trait and concrete implementations.
// Mirrors the C++ Emulation hierarchy.

use crate::key_condition::KeyCondition;

/// Trait that KeyEmulator exposes to Emulation objects for key injection.
pub trait KeyEmulatorApi {
    fn emulate_key(&self, vkey: u8, up: bool);
}

/// An emulation action — maps a key condition to injected keystrokes.
pub trait Emulation {
    fn execute_down(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition);
    fn execute_up(&self, emulator: &dyn KeyEmulatorApi, cond: &KeyCondition);
}

// --- NormalKeyEmulation ---

/// Injects a single virtual key.
pub struct NormalKeyEmulation {
    vkey: u8,
}

impl NormalKeyEmulation {
    pub fn new(vkey: u8) -> Self {
        Self { vkey }
    }
}

impl Emulation for NormalKeyEmulation {
    fn execute_down(&self, emulator: &dyn KeyEmulatorApi, _cond: &KeyCondition) {
        emulator.emulate_key(self.vkey, false);
    }

    fn execute_up(&self, emulator: &dyn KeyEmulatorApi, _cond: &KeyCondition) {
        emulator.emulate_key(self.vkey, true);
    }
}

// --- NopEmulation ---

/// Does nothing — swallows the key event.
pub struct NopEmulation;

impl Emulation for NopEmulation {
    fn execute_down(&self, _emulator: &dyn KeyEmulatorApi, _cond: &KeyCondition) {}
    fn execute_up(&self, _emulator: &dyn KeyEmulatorApi, _cond: &KeyCondition) {}
}
