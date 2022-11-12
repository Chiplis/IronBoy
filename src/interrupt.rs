use crate::interrupt::InterruptId::{Input, Serial, Stat, Timing, VBlank};
use crate::mmu::MemoryArea;

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub enum InterruptId {
    VBlank = 0x40,
    Stat = 0x48,
    Timing = 0x50,
    Serial = 0x58,
    Input = 0x60,
}

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
                self.enable = value | 0xE0;
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

    fn is_active(&self, mask: u8) -> bool {
        (self.enable & self.flag & mask) != 0
    }

    pub fn triggered(&self, interrupt: InterruptId) -> bool {
        // Lower priority interrupts are ORed with higher priority ones
        let mask = match interrupt {
            VBlank => 0x01,
            Stat => 0x03,
            Timing => 0x07,
            Serial => 0x0F,
            Input => 0x1F,
        };
        self.is_active(mask)
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
