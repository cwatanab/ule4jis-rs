// US keyboard → JIS layout emulation strategy.

use std::collections::HashMap;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_OEM_1, VK_OEM_102, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_MINUS,
    VK_OEM_PLUS,
};

use crate::decorator::{PressAndReleaseDecorator, ShiftPressDecorator, ShiftReleaseDecorator};
use crate::emulation::{Emulation, NopEmulation, NormalKeyEmulation};
use crate::key_condition::KeyCondition;

const VK_OEM_AUTO: u8 = 0xF3;
const VK_OEM_ENLW: u8 = 0xF4;

/// Fast lookup: `cmp_value() → Emulation`. Keyed by packed u16 so lookups
/// never require cloning a `KeyCondition`.
pub type EmulationMap = HashMap<u16, Box<dyn Emulation>>;

pub trait EmulationStrategy {
    fn build_map(&self, dest: &mut EmulationMap);
}

pub struct USonJISStrategy;

impl EmulationStrategy for USonJISStrategy {
    fn build_map(&self, dest: &mut EmulationMap) {
        dest.clear();

        // @ — US Shift+2 → JIS @, release shift
        dest.insert(
            KeyCondition::with_shift(b'2', true).cmp_value(),
            Box::new(ShiftReleaseDecorator::new(Box::new(
                NormalKeyEmulation::new(VK_OEM_3.0 as u8),
            ))),
        );

        // ^ — US Shift+6 → JIS ^, release shift
        dest.insert(
            KeyCondition::with_shift(b'6', true).cmp_value(),
            Box::new(ShiftReleaseDecorator::new(Box::new(
                NormalKeyEmulation::new(VK_OEM_7.0 as u8),
            ))),
        );

        // & — US Shift+7 → JIS 6 key
        dest.insert(
            KeyCondition::with_shift(b'7', true).cmp_value(),
            Box::new(NormalKeyEmulation::new(b'6')),
        );

        // * — US Shift+8 → JIS : key
        dest.insert(
            KeyCondition::with_shift(b'8', true).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_1.0 as u8)),
        );

        // ( — US Shift+9 → JIS 8 key
        dest.insert(
            KeyCondition::with_shift(b'9', true).cmp_value(),
            Box::new(NormalKeyEmulation::new(b'8')),
        );

        // ) — US Shift+0 → JIS 9 key
        dest.insert(
            KeyCondition::with_shift(b'0', true).cmp_value(),
            Box::new(NormalKeyEmulation::new(b'9')),
        );

        // _ — US Shift+- → JIS \
        dest.insert(
            KeyCondition::with_shift(VK_OEM_MINUS.0 as u8, true).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_102.0 as u8)),
        );

        // = — US = key → Shift + JIS -
        dest.insert(
            KeyCondition::with_shift(VK_OEM_7.0 as u8, false).cmp_value(),
            Box::new(ShiftPressDecorator::new(Box::new(NormalKeyEmulation::new(
                VK_OEM_MINUS.0 as u8,
            )))),
        );

        // + — US Shift+= → JIS +
        dest.insert(
            KeyCondition::with_shift(VK_OEM_7.0 as u8, true).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_PLUS.0 as u8)),
        );

        // ` — US ` (VK_OEM_AUTO) → nop
        dest.insert(
            KeyCondition::with_shift(VK_OEM_AUTO, false).cmp_value(),
            Box::new(NopEmulation),
        );

        // ` — US ` via VK_OEM_ENLW → press-and-release Shift + VK_OEM_3
        dest.insert(
            KeyCondition::with_shift(VK_OEM_ENLW, false).cmp_value(),
            Box::new(PressAndReleaseDecorator::new(Box::new(
                ShiftPressDecorator::new(Box::new(NormalKeyEmulation::new(VK_OEM_3.0 as u8))),
            ))),
        );

        // ~ — US Shift+` (VK_OEM_AUTO) → nop
        dest.insert(
            KeyCondition::with_shift(VK_OEM_AUTO, true).cmp_value(),
            Box::new(NopEmulation),
        );

        // ~ — US Shift+` via VK_OEM_ENLW → press-and-release VK_OEM_7
        dest.insert(
            KeyCondition::with_shift(VK_OEM_ENLW, true).cmp_value(),
            Box::new(PressAndReleaseDecorator::new(Box::new(
                NormalKeyEmulation::new(VK_OEM_7.0 as u8),
            ))),
        );

        // [ — US [ → JIS [
        dest.insert(
            KeyCondition::with_shift(VK_OEM_3.0 as u8, false).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_4.0 as u8)),
        );

        // ] — US ] → JIS ]
        dest.insert(
            KeyCondition::with_shift(VK_OEM_4.0 as u8, false).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_6.0 as u8)),
        );

        // { — US Shift+[ → JIS [
        dest.insert(
            KeyCondition::with_shift(VK_OEM_3.0 as u8, true).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_4.0 as u8)),
        );

        // } — US Shift+] → JIS ]
        dest.insert(
            KeyCondition::with_shift(VK_OEM_4.0 as u8, true).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_6.0 as u8)),
        );

        // : — US Shift+= → JIS : without shift
        dest.insert(
            KeyCondition::with_shift(VK_OEM_PLUS.0 as u8, true).cmp_value(),
            Box::new(ShiftReleaseDecorator::new(Box::new(
                NormalKeyEmulation::new(VK_OEM_1.0 as u8),
            ))),
        );

        // ' — US ; → JIS 7 with Shift
        dest.insert(
            KeyCondition::with_shift(VK_OEM_1.0 as u8, false).cmp_value(),
            Box::new(ShiftPressDecorator::new(Box::new(NormalKeyEmulation::new(
                b'7',
            )))),
        );

        // " — US Shift+; → JIS 2
        dest.insert(
            KeyCondition::with_shift(VK_OEM_1.0 as u8, true).cmp_value(),
            Box::new(NormalKeyEmulation::new(b'2')),
        );

        // \ — US \ → JIS \
        dest.insert(
            KeyCondition::with_shift(VK_OEM_6.0 as u8, false).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_102.0 as u8)),
        );

        // | — US Shift+\ → JIS |
        dest.insert(
            KeyCondition::with_shift(VK_OEM_6.0 as u8, true).cmp_value(),
            Box::new(NormalKeyEmulation::new(VK_OEM_5.0 as u8)),
        );
    }
}
