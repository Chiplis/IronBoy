use crate::serial::State::{Off, Transfer};

#[derive(PartialEq, Eq)]
pub enum State {
    Off,
    Transfer(u8),
}

pub struct LinkCable {
    pub(crate) data: u8,
    pub(crate) control: u8,
    pub(crate) transfer: State,
}

impl LinkCable {
    pub(crate) fn new() -> Self {
        LinkCable {
            data: 0,
            control: 0,
            transfer: Off,
        }
    }

    fn set_control(&mut self, control: u8) {
        self.control = control;
        self.transfer = Transfer(0);
        if self.control & 1 == 1 {
            self.data = 0xFF;
            self.control &= 0x7F;
        }
    }

    pub(crate) fn machine_cycle(&mut self) -> bool {
        if self.control & 1 != 1 {
            return false;
        }

        self.transfer = match self.transfer {
            Transfer(x) => Transfer(x + 1),
            Off => Off,
        };

        if self.transfer != Transfer(8) {
            false
        } else {
            self.transfer = Off;
            true
        }
    }

    pub(crate) fn read(&self, address: usize) -> Option<u8> {
        match address {
            0xFF01 => Some(self.data),
            0xFF02 => Some(self.control),
            _ => None,
        }
    }

    pub(crate) fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0xFF01 => self.data = value,
            0xFF02 => self.set_control(value),
            _ => return false,
        }
        true
    }
}
