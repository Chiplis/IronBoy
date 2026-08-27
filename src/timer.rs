use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct Timer {
    tima: u8,
    tma: u8,
    tac: u8,
    ticks: u16,
    #[serde(default)]
    divider_written: bool,
    interrupt: bool,
    #[serde(default)]
    interrupt_from_write: bool,
    interrupt_served: bool,
}

impl MemoryArea for Timer {
    fn read(&self, address: usize) -> Option<u8> {
        match address {
            Timer::DIVIDER => Some(self.ticks.to_le_bytes()[1]),
            Timer::TIMA => Some(self.tima),
            Timer::TMA => Some(self.tma),
            Timer::TAC => Some(self.tac | 0xF8),
            _ => None,
        }
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            Timer::DIVIDER => {
                let old_ticks = self.ticks;
                self.ticks = 0x00;
                self.divider_written = true;
                if self.timer_enabled() && self.timer_increase(old_ticks) {
                    self.increment_tima_from_write();
                }
            }
            Timer::TIMA => {
                if !self.interrupt_served {
                    self.tima = value;
                    self.interrupt = false;
                }
            }
            Timer::TMA => {
                self.tma = value;
                if self.interrupt_served {
                    self.tima = value
                }
            }
            Timer::TAC => {
                let old_signal = self.timer_signal();
                self.tac = value & 0x07;
                if old_signal && !self.timer_signal() {
                    self.increment_tima_from_write();
                }
            }
            _ => return false,
        };
        true
    }
}

impl Timer {
    const DIVIDER: usize = 0xFF04;
    const TIMA: usize = 0xFF05;
    const TMA: usize = 0xFF06;
    const TAC: usize = 0xFF07;

    pub fn new(boot_rom: bool) -> Self {
        Self {
            tima: 0,
            tma: 0,
            tac: 0,
            ticks: if boot_rom { 0x00 } else { 0xABCC },
            divider_written: false,
            interrupt: false,
            interrupt_from_write: false,
            interrupt_served: false,
        }
    }

    pub fn machine_cycle(&mut self, ticks: u16) -> bool {
        self.interrupt_served = false;

        let interrupt = self.interrupt && !self.interrupt_from_write;

        if self.interrupt_from_write {
            self.interrupt_from_write = false;
        }

        if interrupt {
            self.tima = self.tma;
            self.interrupt_served = true;
        }

        if interrupt {
            self.interrupt = false;
        }

        let old_ticks = self.ticks;
        self.ticks = self.ticks.wrapping_add(ticks);
        self.tima_increase(old_ticks);

        interrupt
    }

    fn tima_increase(&mut self, old_ticks: u16) {
        if self.timer_enabled() && self.timer_increase(old_ticks) {
            self.increment_tima();
        }
    }

    fn increment_tima(&mut self) {
        let (new_tima, overflow) = self.tima.overflowing_add(1);
        self.tima = new_tima;
        self.interrupt = overflow;
    }

    fn increment_tima_from_write(&mut self) {
        self.increment_tima();
        // An overflow caused by a DIV/TAC write is not visible to the CPU in
        // the same machine cycle as the write. Preserve it for one cycle so
        // interrupt sampling and TIMA reload occur at the hardware boundary.
        self.interrupt_from_write = self.interrupt;
    }

    fn timer_signal(&self) -> bool {
        self.timer_enabled() && self.ticks & self.frequency() != 0
    }

    fn timer_increase(&self, old_timer: u16) -> bool {
        old_timer & self.frequency() != 0 && self.ticks & self.frequency() == 0
    }

    fn timer_enabled(&self) -> bool {
        self.tac & 0x04 != 0
    }

    fn frequency(&self) -> u16 {
        2_u16.pow(match self.tac & 0x03 {
            0x03 => 7,
            0x02 => 5,
            0x01 => 3,
            0x00 => 9,
            _ => unreachable!(),
        })
    }

    pub(crate) fn divider_written(&self) -> bool {
        self.divider_written
    }
}
