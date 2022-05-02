use crate::interrupt::InterruptId::{JoypadInt, SerialInt, StatInt, TimerInt, VBlankInt};
use crate::interrupt::InterruptState::{Active, Enabled, Inactive, Priority, Requested};
use std::collections::HashMap;
use std::ops::Index;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterruptId {
    VBlankInt = 0x40,
    StatInt = 0x48,
    TimerInt = 0x50,
    SerialInt = 0x58,
    JoypadInt = 0x60,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterruptState {
    Active,
    Inactive,
    Enabled,
    Requested,
    Priority(InterruptId),
}

pub struct InterruptHandler {
    registers: HashMap<usize, u8>,
    vblank: InterruptMask,
    stat: InterruptMask,
    serial: InterruptMask,
    timer: InterruptMask,
    joypad: InterruptMask,
}

pub const IE_ADDRESS: usize = 0xFFFF;
pub const IF_ADDRESS: usize = 0xFF0F;

impl InterruptHandler {
    pub fn new() -> Self {
        let mut registers = HashMap::new();
        registers.insert(IF_ADDRESS, 0x00);
        registers.insert(IE_ADDRESS, 0x00);
        let vblank = InterruptMask(0x01);
        let stat = InterruptMask(0x02);
        let timer = InterruptMask(0x04);
        let serial = InterruptMask(0x08);
        let joypad = InterruptMask(0x10);
        InterruptHandler {
            registers,
            vblank,
            stat,
            timer,
            serial,
            joypad,
        }
    }

    pub fn get_state(&self, interrupt: InterruptId) -> InterruptState {
        let ie_flag = self.registers[&IE_ADDRESS];
        let if_flag = self.registers[&IF_ADDRESS];
        let enabled = ie_flag & self[interrupt].0 != 0;
        let requested = if_flag & self[interrupt].0 != 0;
        let active = requested && enabled;
        let state = if active {
            Active
        } else if enabled {
            Enabled
        } else if requested {
            Requested
        } else {
            Inactive
        };
        match interrupt {
            VBlankInt => state,
            StatInt => {
                if self.get_state(VBlankInt) != Active {
                    state
                } else {
                    Priority(VBlankInt)
                }
            }
            TimerInt => {
                if self.get_state(StatInt) != Active {
                    state
                } else {
                    Priority(StatInt)
                }
            }
            SerialInt => {
                if self.get_state(TimerInt) != Active {
                    state
                } else {
                    Priority(TimerInt)
                }
            }
            JoypadInt => {
                if self.get_state(SerialInt) != Active {
                    state
                } else {
                    Priority(SerialInt)
                }
            }
        }
    }

    pub fn set(&mut self, interrupts: Vec<InterruptId>, set: bool) {
        if set {
            interrupts
                .iter()
                .for_each(|i| *self.registers.get_mut(&IF_ADDRESS).unwrap() |= self[*i].0)
        } else {
            interrupts
                .iter()
                .for_each(|i| *self.registers.get_mut(&IF_ADDRESS).unwrap() &= !self[*i].0)
        }
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        self.registers.get(&address).copied()
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        if !self.registers.contains_key(&address) {
            return false;
        }
        self.registers.insert(address, value | 0xE0);
        true
    }
}

pub struct InterruptMask(u8);

impl Index<InterruptId> for InterruptHandler {
    type Output = InterruptMask;

    fn index(&self, id: InterruptId) -> &Self::Output {
        match id {
            VBlankInt => &self.vblank,
            StatInt => &self.stat,
            TimerInt => &self.timer,
            SerialInt => &self.serial,
            JoypadInt => &self.joypad,
        }
    }
}
