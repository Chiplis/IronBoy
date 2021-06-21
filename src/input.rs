pub struct TimerInterrupt();

use minifb::{InputCallback, Key};
use crate::memory_map::MemoryMap;
use core::time;
use std::thread;

pub struct Input {
    trigger_interrupt: bool,
    register: u8
}


impl InputCallback for &mut Input {
    fn add_char(&mut self, uni_char: u32) {
        match char::from_u32(uni_char) {
            Some('d' | 'c') => {
                self.trigger_interrupt = true;
                self.register = self.register & !0x01
            }
            Some('a' | 'z') => {
                self.trigger_interrupt = true;
                self.register = self.register & !0x02
            }
            Some('w' | 'l') => {
                self.trigger_interrupt = true;
                self.register = self.register & !0x04
            }
            Some('s' | 'k') => {
                self.trigger_interrupt = true;
                self.register = self.register & !0x08
            }
            _ => {}
        }
    }
}

pub struct InputInterrupt();

impl Input {

    pub fn new() -> Self {
        Self { register: 0xEF, trigger_interrupt: false }
    }

    pub fn input_cycle(&mut self) -> Option<InputInterrupt> {
        let mut interrupt = None;
        if self.trigger_interrupt {
            self.trigger_interrupt = false;
            interrupt = Some(InputInterrupt())
        }
        interrupt
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        match address {
            0xFF00 => Some(self.register),
            _ => None
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0xFF00 => {
                self.register = (value & 0xF0) |
                    (self.register & 0x01) |
                    (self.register & 0x02) |
                    (self.register & 0x04) |
                    (self.register & 0x08);
            }
            _ => return false
        };
        true
    }
}