// Key state tracking for emulation map lookup.
// Mirrors the C++ KeyCondition class.

use std::cmp::Ordering;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
};

/// Modifier keys tracked by KeyCondition.
const MOD_KEYS: [u16; 6] = [
    VK_LSHIFT.0,
    VK_RSHIFT.0,
    VK_LCONTROL.0,
    VK_RCONTROL.0,
    VK_LMENU.0,
    VK_RMENU.0,
];

/// Represents the state of a single key press: which virtual key,
/// plus whether Shift/Alt/Ctrl are held.
#[derive(Clone, Debug)]
pub struct KeyCondition {
    /// The virtual key code of the pressed key.
    pub last_vkey: u8,
    /// Modifier key held state, indexed by position in MOD_KEYS.
    mod_key_state: [bool; 6],
}

impl KeyCondition {
    /// Create an empty key condition (no key, no mods).
    pub fn new() -> Self {
        Self {
            last_vkey: 0,
            mod_key_state: [false; 6],
        }
    }

    /// Create a key condition for `vkey`, optionally with Shift held.
    pub fn with_shift(vkey: u8, shift: bool) -> Self {
        let mut kc = Self::new();
        kc.last_vkey = vkey;
        if shift {
            if let Some(idx) = mod_key_index(VK_LSHIFT.0) {
                kc.mod_key_state[idx] = true;
            }
        }
        kc
    }

    /// Update internal state when a key is pressed or released.
    pub fn change_key_state(&mut self, vkey: u8, up: bool) {
        self.last_vkey = vkey;
        if let Some(idx) = mod_key_index(vkey as u16) {
            self.mod_key_state[idx] = !up;
        }
    }

    /// Get the held state of a specific modifier key.
    pub fn get_mod_key_state(&self, vkey: u16) -> bool {
        if let Some(idx) = mod_key_index(vkey) {
            self.mod_key_state[idx]
        } else {
            false
        }
    }

    /// Build a packed comparison value: shift | alt | vkey.
    /// Ignores left/right distinction of modifier keys.
    pub(crate) fn cmp_value(&self) -> u16 {
        let shift = self.get_mod_key_state(VK_LSHIFT.0) || self.get_mod_key_state(VK_RSHIFT.0);
        let alt = self.get_mod_key_state(VK_LMENU.0) || self.get_mod_key_state(VK_RMENU.0);

        let mut val: u16 = 0;
        val |= if shift { 1 } else { 0 };
        val <<= 1;
        val |= if alt { 1 } else { 0 };
        val <<= 8;
        val |= self.last_vkey as u16;
        val
    }
}

fn mod_key_index(vkey: u16) -> Option<usize> {
    MOD_KEYS.iter().position(|&k| k == vkey)
}

// --- Ord / PartialOrd / Eq / PartialEq ---

impl PartialEq for KeyCondition {
    fn eq(&self, other: &Self) -> bool {
        self.cmp_value() == other.cmp_value()
    }
}

impl Eq for KeyCondition {}

impl PartialOrd for KeyCondition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyCondition {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_value().cmp(&other.cmp_value())
    }
}
