use crate::interrupt::InterruptId::{Input, Serial, Stat, Timing, VBlank};
use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub enum InterruptId {
    VBlank = 0x40,
    Stat = 0x48,
    Timing = 0x50,
    Serial = 0x58,
    Input = 0x60,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct InterruptHandler {
    flag: u8,
    enable: u8,
}

pub const IE_ADDRESS: usize = 0xFFFF;
pub const IF_ADDRESS: usize = 0xFF0F;

impl MemoryArea for InterruptHandler {
    fn read(&self, address: usize) -> Option<u8> {
        match address {
            IE_ADDRESS => Some(self.enable),
            IF_ADDRESS => Some(self.flag),
            _ => None,
        }
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            IE_ADDRESS => {
                self.enable = value;
                true
            }
            IF_ADDRESS => {
                self.flag = value | 0xE0;
                true
            }
            _ => false,
        }
    }
}

impl InterruptHandler {
    pub fn new() -> Self {
        let flag = 0x00;
        let enable = 0x00;
        InterruptHandler { flag, enable }
    }

    pub fn highest_pending(&self) -> Option<InterruptId> {
        match self.enable & self.flag & 0x1F {
            pending if pending & 0x01 != 0 => Some(VBlank),
            pending if pending & 0x02 != 0 => Some(Stat),
            pending if pending & 0x04 != 0 => Some(Timing),
            pending if pending & 0x08 != 0 => Some(Serial),
            pending if pending & 0x10 != 0 => Some(Input),
            _ => None,
        }
    }

    fn mask(interrupt: InterruptId) -> u8 {
        match interrupt {
            VBlank => 0x01,
            Stat => 0x02,
            Timing => 0x04,
            Serial => 0x08,
            Input => 0x10,
        }
    }

    pub fn set(&mut self, interrupt: InterruptId) {
        self.flag |= Self::mask(interrupt)
    }

    pub fn unset(&mut self, interrupt: InterruptId) {
        self.flag &= !Self::mask(interrupt)
    }
}
