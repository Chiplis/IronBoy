use crate::interrupt::InterruptId::{Input, Serial, Stat, Timing, VBlank};
use crate::interrupt::InterruptState::{Active, Enabled, Inactive, Requested};

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub enum InterruptId {
    VBlank = 0x40,
    Stat = 0x48,
    Timing = 0x50,
    Serial = 0x58,
    Input = 0x60,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterruptState {
    Active,
    Inactive,
    Enabled,
    Requested,
}

pub struct InterruptHandler {
    flag: u8,
    enable: u8,
}

pub const IE_ADDRESS: usize = 0xFFFF;
pub const IF_ADDRESS: usize = 0xFF0F;

impl InterruptHandler {
    pub fn new() -> Self {
        let flag = 0x00;
        let enable = 0x00;
        InterruptHandler { flag, enable }
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

    fn calc_state(&self, interrupt: InterruptId) -> InterruptState {
        let mask = Self::mask(interrupt);
        let enabled = self.enable & mask != 0;
        let requested = self.flag & mask != 0;
        match (requested, enabled) {
            (true, true) => Active,
            (true, false) => Requested,
            (false, true) => Enabled,
            (false, false) => Inactive,
        }
    }

    pub fn get_state(&self, interrupt: InterruptId) -> InterruptState {
        let inter = Self::mask(interrupt);
        if inter > VBlank as u8 && self.calc_state(VBlank) == Active
            || inter > Stat as u8 && self.calc_state(Stat) == Active
            || inter > Timing as u8 && self.calc_state(Timing) == Active
            || inter > Serial as u8 && self.calc_state(Serial) == Active
        {
            Active
        } else {
            self.calc_state(interrupt)
        }
    }

    pub fn set(&mut self, interrupt: InterruptId) {
        self.flag |= Self::mask(interrupt)
    }

    pub fn unset(&mut self, interrupt: InterruptId) {
        self.flag &= !Self::mask(interrupt)
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        match address {
            IE_ADDRESS => Some(self.enable),
            IF_ADDRESS => Some(self.flag),
            _ => None,
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
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
