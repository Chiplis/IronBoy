use crate::interrupt::InterruptId::{VBlank, STAT, Timer, Serial, Joypad};
use crate::memory_map::MemoryRegion;
use std::ops::{Range, RangeInclusive, Index};
use std::collections::HashMap;
use crate::interrupt::InterruptState::{Enabled, Requested, Active, Inactive, Priority};


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterruptId {
    VBlank = 0x40,
    STAT = 0x48,
    Timer = 0x50,
    Serial = 0x58,
    Joypad = 0x60,
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
    registers: HashMap<u16, u8>,
    vblank: InterruptMask,
    stat: InterruptMask,
    serial: InterruptMask,
    timer: InterruptMask,
    joypad: InterruptMask,
    invalid: [u8; 1]
}

impl Interrupt {
    const IE_ADDRESS: u16 = 0xFFFF;
    const IF_ADDRESS: u16 = 0xFF0F;
    const JOYPAD_ADDRESS: u16 = 0xFF00;

    pub fn new() -> Self {
        let mut registers = HashMap::new();
        registers.insert(Interrupt::IF_ADDRESS, 0x0);
        registers.insert(Interrupt::IE_ADDRESS, 0x0);
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
            VBlank => state,
            STAT => if self.state(VBlank) != Active { state } else { Priority(VBlank) },
            Timer => if self.state(STAT) != Active { state } else { Priority(STAT) },
            Serial => if self.state(Timer) != Active { state } else { Priority(Timer) },
            Joypad => if self.state(Serial) != Active { state } else { Priority(Joypad) },
        }
    }

    pub fn set(&mut self, interrupt: InterruptId, set: bool) {
        if set {
            *self.read_mut(Interrupt::IF_ADDRESS) |= self[interrupt].0;
        } else {
            *self.read_mut(Interrupt::IF_ADDRESS) &= !self[interrupt].0;
        }
    }
}

pub struct InterruptMask(u8);

impl Index<InterruptId> for Interrupt {
    type Output = InterruptMask;

    fn index(&self, id: InterruptId) -> &Self::Output {
        match id {
            VBlank => &self.vblank,
            STAT => &self.stat,
            Timer => &self.timer,
            Serial => &self.serial,
            Joypad => &self.joypad,
        }
    }
}

impl MemoryRegion for Interrupt {
    fn sub_regions(&self) -> Vec<RangeInclusive<u16>> {
        vec![
            (Interrupt::IF_ADDRESS..=Interrupt::IF_ADDRESS),
            (Interrupt::IE_ADDRESS..=Interrupt::IE_ADDRESS),
            // (Interrupt::JOYPAD_ADDRESS..=Interrupt::JOYPAD_ADDRESS)
        ]
    }

    fn read(&self, address: u16) -> &u8 {
        if address == Interrupt::JOYPAD_ADDRESS {
            &0xEF
        } else {
            self.registers.get(&address).unwrap()
        }
    }

    fn read_mut(&mut self, address: u16) -> &mut u8 {
        if address == Interrupt::JOYPAD_ADDRESS {
            &mut self.invalid[0]
        } else {
            self.registers.get_mut(&address).unwrap()
        }
    }
}
