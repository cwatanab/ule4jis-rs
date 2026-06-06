use crate::key_condition::KeyCondition;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LSHIFT, VK_RSHIFT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emulation {
    Normal { vkey: u8 },
    Nop,
    ShiftPress { vkey: u8 },
    ShiftRelease { vkey: u8 },
    PressAndRelease { vkey: u8 },
    PressAndReleaseShift { vkey: u8 },
}

impl Emulation {
    pub fn execute(&self, up: bool, cond: &KeyCondition, mut inject: impl FnMut(u8, bool)) {
        match self {
            Emulation::Normal { vkey } => {
                inject(*vkey, up);
            }
            Emulation::Nop => {}
            Emulation::ShiftPress { vkey } => {
                if !up {
                    inject(VK_LSHIFT.0 as u8, false);
                    inject(*vkey, false);
                    inject(VK_LSHIFT.0 as u8, true);
                } else {
                    inject(*vkey, true);
                }
            }
            Emulation::ShiftRelease { vkey } => {
                if !up {
                    let lshift = cond.get_mod_key_state(VK_LSHIFT.0);
                    let rshift = cond.get_mod_key_state(VK_RSHIFT.0);
                    if lshift {
                        inject(VK_LSHIFT.0 as u8, true);
                    }
                    if rshift {
                        inject(VK_RSHIFT.0 as u8, true);
                    }
                    inject(*vkey, false);
                    if lshift {
                        inject(VK_LSHIFT.0 as u8, false);
                    }
                    if rshift {
                        inject(VK_RSHIFT.0 as u8, false);
                    }
                } else {
                    inject(*vkey, true);
                }
            }
            Emulation::PressAndRelease { vkey } => {
                inject(*vkey, false);
                inject(*vkey, true);
            }
            Emulation::PressAndReleaseShift { vkey } => {
                inject(VK_LSHIFT.0 as u8, false);
                inject(*vkey, false);
                inject(VK_LSHIFT.0 as u8, true);
                inject(*vkey, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LS: u8 = VK_LSHIFT.0 as u8;
    const RS: u8 = VK_RSHIFT.0 as u8;
    const L_HELD: u8 = 1;
    const R_HELD: u8 = 2;
    const N: Emulation = Emulation::Nop;

    const fn normal(vkey: u8) -> Emulation {
        Emulation::Normal { vkey }
    }

    const fn shift_press(vkey: u8) -> Emulation {
        Emulation::ShiftPress { vkey }
    }

    const fn shift_release(vkey: u8) -> Emulation {
        Emulation::ShiftRelease { vkey }
    }

    const fn press_and_release(vkey: u8) -> Emulation {
        Emulation::PressAndRelease { vkey }
    }

    const fn press_and_release_shift(vkey: u8) -> Emulation {
        Emulation::PressAndReleaseShift { vkey }
    }

    fn collect(emu: &Emulation, up: bool, cond: &KeyCondition) -> Vec<(u8, bool)> {
        let mut v = Vec::new();
        emu.execute(up, cond, |vkey, key_up| v.push((vkey, key_up)));
        v
    }

    fn condition(shift: u8) -> KeyCondition {
        let mut cond = KeyCondition::default();
        if shift & L_HELD != 0 {
            cond.change_key_state(LS, false);
        }
        if shift & R_HELD != 0 {
            cond.change_key_state(RS, false);
        }
        cond
    }

    #[test]
    fn execute_cases() {
        let cases = [
            (normal(0x41), false, 0, &[(0x41, false)][..]),
            (normal(0x41), true, 0, &[(0x41, true)]),
            (N, false, 0, &[]),
            (
                shift_press(0x41),
                false,
                0,
                &[(LS, false), (0x41, false), (LS, true)],
            ),
            (shift_press(0x41), true, 0, &[(0x41, true)]),
            (shift_release(0x30), false, 0, &[(0x30, false)]),
            (
                shift_release(0x30),
                false,
                L_HELD,
                &[(LS, true), (0x30, false), (LS, false)],
            ),
            (
                shift_release(0x30),
                false,
                R_HELD,
                &[(RS, true), (0x30, false), (RS, false)],
            ),
            (
                shift_release(0x30),
                false,
                L_HELD | R_HELD,
                &[
                    (LS, true),
                    (RS, true),
                    (0x30, false),
                    (LS, false),
                    (RS, false),
                ],
            ),
            (shift_release(0x30), true, 0, &[(0x30, true)]),
            (
                press_and_release(0x41),
                false,
                0,
                &[(0x41, false), (0x41, true)],
            ),
            (
                press_and_release_shift(0x41),
                false,
                0,
                &[(LS, false), (0x41, false), (LS, true), (0x41, true)],
            ),
        ];

        for (i, (emulation, up, shift, expected)) in cases.into_iter().enumerate() {
            assert_eq!(
                collect(&emulation, up, &condition(shift)),
                expected,
                "case {i}"
            );
        }
    }
}
