use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
    VK_SHIFT,
};

const MOD_LSHIFT: u8 = 1 << 0;
const MOD_RSHIFT: u8 = 1 << 1;
const MOD_LCTRL: u8 = 1 << 2;
const MOD_RCTRL: u8 = 1 << 3;
const MOD_LALT: u8 = 1 << 4;
const MOD_RALT: u8 = 1 << 5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyCondition {
    pub last_vkey: u8,
    mod_bits: u8,
}

impl KeyCondition {
    #[cfg(test)]
    pub fn with_shift(vkey: u8, shift: bool) -> Self {
        let mut kc = Self {
            last_vkey: vkey,
            ..Default::default()
        };
        if shift {
            kc.mod_bits |= MOD_LSHIFT;
        }
        kc
    }

    pub fn change_key_state(&mut self, vkey: u8, up: bool) {
        self.last_vkey = vkey;
        if let Some(bit) = mod_key_bit(vkey as u16) {
            if up {
                self.mod_bits &= !bit;
            } else {
                self.mod_bits |= bit;
            }
        }
    }

    pub fn get_mod_key_state(&self, vkey: u16) -> bool {
        mod_key_bit(vkey).is_some_and(|bit| self.mod_bits & bit != 0)
    }

    pub(crate) fn cmp_value(&self) -> u16 {
        let shift = self.mod_bits & (MOD_LSHIFT | MOD_RSHIFT) != 0;
        let alt = self.mod_bits & (MOD_LALT | MOD_RALT) != 0;
        ((shift as u16) << 9) | ((alt as u16) << 8) | self.last_vkey as u16
    }
}

fn mod_key_bit(vkey: u16) -> Option<u8> {
    match vkey {
        v if v == VK_LSHIFT.0 || v == VK_SHIFT.0 => Some(MOD_LSHIFT),
        v if v == VK_RSHIFT.0 => Some(MOD_RSHIFT),
        v if v == VK_LCONTROL.0 || v == VK_CONTROL.0 => Some(MOD_LCTRL),
        v if v == VK_RCONTROL.0 => Some(MOD_RCTRL),
        v if v == VK_LMENU.0 || v == VK_MENU.0 => Some(MOD_LALT),
        v if v == VK_RMENU.0 => Some(MOD_RALT),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmp_value_cases() {
        let cases = [
            (b'A', &[][..], 0x0041),
            (b'2', &[VK_LSHIFT.0 as u8], 0x0232),
            (b'2', &[VK_RSHIFT.0 as u8], 0x0232),
            (0x10, &[VK_LMENU.0 as u8], 0x0110),
            (0x05, &[VK_LSHIFT.0 as u8, VK_RMENU.0 as u8], 0x0305),
        ];

        for (last_vkey, held, expected) in cases {
            let mut kc = KeyCondition::default();
            for vkey in held {
                kc.change_key_state(*vkey, false);
            }
            kc.last_vkey = last_vkey;
            assert_eq!(kc.cmp_value(), expected);
        }
    }

    #[test]
    fn modifier_state_cases() {
        let mut kc = KeyCondition::with_shift(b'2', true);
        assert!(kc.get_mod_key_state(VK_LSHIFT.0));
        assert_eq!(kc.last_vkey, b'2');

        kc.change_key_state(VK_LSHIFT.0 as u8, true);
        assert!(!kc.get_mod_key_state(VK_LSHIFT.0));
        assert!(!KeyCondition::with_shift(b'8', false).get_mod_key_state(VK_LSHIFT.0));
        assert!(!kc.get_mod_key_state(0xFFFF));
    }
}
