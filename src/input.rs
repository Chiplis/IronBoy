pub struct TimerInterrupt();

use minifb::{InputCallback, Key};
use crate::memory_map::MemoryMap;
use core::time;
use std::thread;
use std::sync::mpsc::{Sender, Receiver};

pub struct InputReceiver {
    trigger_interrupt: bool,
    register: u8,
    input_receiver: Receiver<char>
}

pub struct InputSender {
    sender: Sender<char>
}

impl InputSender {
    pub(crate) fn new(sender: Sender<char>) -> Self { Self { sender } }
}

impl InputCallback for InputSender {
    fn add_char(&mut self, uni_char: u32) {
        match char::from_u32(uni_char) {
            Some(n @ ('d' | 'c' | 'a' | 'z' | 'w' | 'l' | 's' | 'k')) => { self.sender.send(n); },
            _ => {}
        }
    }
}

pub struct InputInterrupt();

impl InputReceiver {

    pub fn new(input_receiver: Receiver<char>) -> Self {
        Self { input_receiver, register: 0xEF, trigger_interrupt: false }
    }

    pub fn input_cycle(&mut self) -> Option<InputInterrupt> {
        let mut interrupt = None;
        for r in &mut self.input_receiver.try_iter() {
            self.trigger_interrupt = true;
            match r {
                'd' | 'c' => self.register &= !0x01,
                'a' | 'z' => self.register &= !0x02,
                'w' | 'l' => self.register &= !0x04,
                's' | 'k' => self.register &= !0x08,
                _ => self.trigger_interrupt = false
            }
        }
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