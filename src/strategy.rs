use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_OEM_1, VK_OEM_102, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_MINUS,
    VK_OEM_PLUS,
};

use crate::emulation::Emulation;

const VK_OEM_AUTO: u8 = 0xF3;
const VK_OEM_ENLW: u8 = 0xF4;

const EMULATION_MAP_LEN: usize = 1024;

pub type EmulationMap = [Option<Emulation>; EMULATION_MAP_LEN];
pub const US_ON_JIS_MAP: EmulationMap = build_us_on_jis_map();

#[derive(Clone, Copy)]
struct Mapping {
    source: u8,
    shift: bool,
    emulation: Emulation,
}

#[rustfmt::skip]
const MAPPINGS: [Mapping; 22] = [
    shift_release(b'2', true, VK_OEM_3.0 as u8), shift_release(b'6', true, VK_OEM_7.0 as u8),
    normal(b'7', true, b'6'), normal(b'8', true, VK_OEM_1.0 as u8),
    normal(b'9', true, b'8'), normal(b'0', true, b'9'),
    normal(VK_OEM_MINUS.0 as u8, true, VK_OEM_102.0 as u8), shift_press(VK_OEM_7.0 as u8, false, VK_OEM_MINUS.0 as u8),
    normal(VK_OEM_7.0 as u8, true, VK_OEM_PLUS.0 as u8), nop(VK_OEM_AUTO, false),
    press_and_release_shift(VK_OEM_ENLW, false, VK_OEM_3.0 as u8), nop(VK_OEM_AUTO, true),
    press_and_release(VK_OEM_ENLW, true, VK_OEM_7.0 as u8), normal(VK_OEM_3.0 as u8, false, VK_OEM_4.0 as u8),
    normal(VK_OEM_4.0 as u8, false, VK_OEM_6.0 as u8), normal(VK_OEM_3.0 as u8, true, VK_OEM_4.0 as u8),
    normal(VK_OEM_4.0 as u8, true, VK_OEM_6.0 as u8), shift_release(VK_OEM_PLUS.0 as u8, true, VK_OEM_1.0 as u8),
    shift_press(VK_OEM_1.0 as u8, false, b'7'), normal(VK_OEM_1.0 as u8, true, b'2'),
    normal(VK_OEM_6.0 as u8, false, VK_OEM_102.0 as u8), normal(VK_OEM_6.0 as u8, true, VK_OEM_5.0 as u8),
];

const fn map(source: u8, shift: bool, emulation: Emulation) -> Mapping {
    Mapping {
        source,
        shift,
        emulation,
    }
}

const fn normal(source: u8, shift: bool, vkey: u8) -> Mapping {
    map(source, shift, Emulation::Normal { vkey })
}

const fn nop(source: u8, shift: bool) -> Mapping {
    map(source, shift, Emulation::Nop)
}

const fn shift_press(source: u8, shift: bool, vkey: u8) -> Mapping {
    map(source, shift, Emulation::ShiftPress { vkey })
}

const fn shift_release(source: u8, shift: bool, vkey: u8) -> Mapping {
    map(source, shift, Emulation::ShiftRelease { vkey })
}

const fn press_and_release(source: u8, shift: bool, vkey: u8) -> Mapping {
    map(source, shift, Emulation::PressAndRelease { vkey })
}

const fn press_and_release_shift(source: u8, shift: bool, vkey: u8) -> Mapping {
    map(source, shift, Emulation::PressAndReleaseShift { vkey })
}

const fn build_us_on_jis_map() -> EmulationMap {
    let mut dest = [None; EMULATION_MAP_LEN];
    let mut i = 0;
    while i < MAPPINGS.len() {
        let mapping = MAPPINGS[i];
        dest[map_key(mapping.source, mapping.shift)] = Some(mapping.emulation);
        i += 1;
    }
    dest
}

const fn map_key(source: u8, shift: bool) -> usize {
    (if shift { 0x200 } else { 0 }) | source as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_declared_mappings_are_present_once() {
        let map = US_ON_JIS_MAP;
        assert_eq!(map.iter().flatten().count(), MAPPINGS.len());
        for mapping in MAPPINGS {
            assert_eq!(
                map[map_key(mapping.source, mapping.shift)],
                Some(mapping.emulation)
            );
        }
    }
}
