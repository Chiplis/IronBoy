use crate::interrupt::InterruptId::{VBlankInt, StatInt, TimerInt, SerialInt, JoypadInt};
use std::ops::{Range, RangeInclusive, Index};
use std::collections::HashMap;
use crate::interrupt::InterruptState::{Enabled, Requested, Active, Inactive, Priority};


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

pub struct Interrupt {
    registers: HashMap<usize, u8>,
    vblank: InterruptMask,
    stat: InterruptMask,
    serial: InterruptMask,
    timer: InterruptMask,
    joypad: InterruptMask,
    invalid: [u8; 1]
}

impl Interrupt {
    const IE_ADDRESS: usize = 0xFFFF;
    const IF_ADDRESS: usize = 0xFF0F;
    const JOYPAD_ADDRESS: usize = 0xFF00;

    pub fn new() -> Self {
        let mut registers = HashMap::new();
        registers.insert(Interrupt::IF_ADDRESS, 0x0);
        registers.insert(Interrupt::IE_ADDRESS, 0x0);
        registers.insert(Interrupt::JOYPAD_ADDRESS, 0xEF);
        let vblank = InterruptMask(0x01);
        let stat = InterruptMask(0x02);
        let timer = InterruptMask(0x04);
        let serial = InterruptMask(0x08);
        let joypad = InterruptMask(0x10);
        let invalid = [1_u8; 1];
        Interrupt { registers, vblank, stat, timer, serial, joypad, invalid}
    }

    pub fn state(&self, interrupt: InterruptId) -> InterruptState {
        let ie_flag = self.registers[&Interrupt::IE_ADDRESS];
        let if_flag = self.registers[&Interrupt::IF_ADDRESS];
        let enabled = ie_flag & self[interrupt].0 != 0;
        let requested = if_flag & self[interrupt].0 != 0;
        let active = requested && enabled;
        let state = if active { Active } else if enabled { Enabled } else if requested { Requested } else { Inactive };
        match interrupt {
            VBlankInt => state,
            StatInt => if self.state(VBlankInt) != Active { state } else { Priority(VBlankInt) },
            TimerInt => if self.state(StatInt) != Active { state } else { Priority(StatInt) },
            SerialInt => if self.state(TimerInt) != Active { state } else { Priority(TimerInt) },
            JoypadInt => if self.state(SerialInt) != Active { state } else { Priority(SerialInt) },
        }
    }

    pub fn set(&mut self, interrupts: Vec<InterruptId>, set: bool) {
        if set {
            interrupts.iter().for_each(|i| *self.registers.get_mut(&Interrupt::IF_ADDRESS).unwrap() |= self[*i].0)
        } else {
            interrupts.iter().for_each(|i| *self.registers.get_mut(&Interrupt::IF_ADDRESS).unwrap() &= !self[*i].0)
        }
    }

    pub(crate) fn read(&self, address: usize) -> Option<&u8> {
        self.registers.get(&address)
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        if address == Interrupt::JOYPAD_ADDRESS {
            self.registers.insert(address, value);
            return true
        }

        if !self.registers.contains_key(&address) { return false }
        self.registers.insert(address, value);
        true
    }
}

pub struct InterruptMask(u8);

impl Index<InterruptId> for Interrupt {
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
