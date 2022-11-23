use std::cmp::max;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use pausable_clock::PausableClock;

use crate::cartridge::Cartridge;
use crate::mbc::MemoryBankController;
use crate::mmu::MemoryArea;

#[derive(Serialize, Deserialize, Debug)]
pub struct MBC3 {
    cartridge: Cartridge,
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u8,
    ram_rtc_bank: u8,
    rom_offset: usize,
    ram_offset: usize,
    ram_enabled: bool,
    expansion_mode: u8,
    rtc: RealTimeClock,
    rtc_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct RealTimeClock {
    #[serde(skip)]
    clock: PausableClock,
    #[serde(skip)]
    additional_secs: u64,
    seconds: u8,
    minutes: u8,
    hours: u8,
    days: u16,
    halted: bool,
    latched: bool,
    day_carry_bit: bool,
    timestamp: SystemTime,
}

impl RealTimeClock {
    fn latch(&mut self, value: u8) {
        match value {
            0 => {
                self.latched = true;
            }
            1 => {
                if self.latched {
                    self.latched = false;
                    let secs = self.clock.now().elapsed_millis() / 1000 + self.additional_secs;
                    self.seconds = (secs % 60) as u8;
                    self.minutes = ((secs / 60) % 60) as u8;
                    self.hours = ((secs / 3600) % 24) as u8;
                    let days = (secs / (3600 * 24)) as u16;
                    self.days = days % 0x1FF;
                    self.day_carry_bit |= days > 0x1FF; // Day carry bit is not reset
                }
            }
            _ => unreachable!(),
        }
    }

    fn read(&self, register: u8) -> u8 {
        match register {
            0x08 => self.seconds,
            0x09 => self.minutes,
            0x0A => self.hours,
            0x0B => (self.days & 0xFF) as u8,
            0x0C => {
                ((self.days >> 8) & 1) as u8
                    | if self.halted { 0x40 } else { 0x00 }
                    | if self.day_carry_bit { 0x80 } else { 0x00 }
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, register: u8, value: u8) {
        if (0x08..=0x0A).contains(&value) {
            if self.halted {
                return;
            }

            let total = self.seconds as u64
                + self.minutes as u64 * 60
                + self.hours as u64 * 3600
                + self.days as u64 * 24 * 3600;

            self.clock = PausableClock::new(Duration::from_secs(total), self.clock.is_paused());
        }

        match register {
            0x08 => self.seconds = value,
            0x09 => self.minutes = value,
            0x0A => self.hours = value,
            0x0B => self.days = (self.days & 0x100) | value as u16,
            0x0C => {
                self.days = value as u16 | if value & 1 == 0 { value as u16 } else { 0x100 };
                self.day_carry_bit = value & 0x80 != 0;
                self.halted = value & 0x40 != 0;
                if self.halted {
                    self.clock.pause();
                } else {
                    self.clock.resume();
                }
            }
            _ => (),
        };
    }
}

impl MBC3 {
    pub fn new(cartridge: Cartridge, rom: Vec<u8>) -> Self {
        Self {
            cartridge,
            rom,
            ram: vec![0; 1024 * 1024 * 2],
            rom_bank: 0,
            ram_rtc_bank: 0,
            rom_offset: 0x4000,
            ram_offset: 0,
            ram_enabled: false,
            expansion_mode: 0,
            rtc: RealTimeClock {
                clock: Default::default(),
                additional_secs: 0,
                seconds: 0,
                minutes: 0,
                hours: 0,
                days: 0,
                halted: false,
                latched: false,
                day_carry_bit: false,
                timestamp: SystemTime::now(),
            },
            rtc_enabled: false,
        }
    }
}

impl MemoryArea for MBC3 {
    fn read(&self, address: usize) -> Option<u8> {
        Some(match address {
            0x0000..=0x3FFF => self.rom[address],
            0x4000..=0x7FFF => self.rom[self.rom_offset + (address & 0x3FFF)],
            0xA000..=0xBFFF if self.ram_enabled && !self.rtc_enabled => {
                self.ram[self.ram_offset + (address & 0x1FFF)]
            }
            0xA000..=0xBFFF if self.ram_enabled => self.rtc.read(self.ram_rtc_bank),
            0xA000..=0xBFFF => 0xFF,
            _ => return None,
        })
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x0000..=0x1FFF => self.ram_enabled = value & 0x0F == 0x0A,
            0x2000..=0x3FFF => {
                self.rom_bank = max(1, value) & 0x7F;
                self.rom_offset = self.rom_bank as usize * 0x4000;
            }
            0x4000..=0x5FFF => {
                if self.expansion_mode != 0 {
                    self.ram_rtc_bank = value;
                    self.rtc_enabled = self.ram_rtc_bank > 0x03;
                    self.ram_offset = self.ram_rtc_bank as usize * 0x2000;
                } else {
                    self.rom_bank = (self.rom_bank & 0x1F) + ((value & 3) << 5);
                    self.rom_offset = self.rom_bank as usize * 0x4000;
                }
            }
            0x6000..=0x7FFF => {
                if self.rtc_enabled {
                    self.rtc.latch(value & 1)
                } else {
                    self.expansion_mode = value & 1
                }
            }
            0xA000..=0xBFFF if self.ram_enabled && !self.rtc_enabled => {
                self.ram[self.ram_offset + (address & 0x1FFF)] = value
            }
            0xA000..=0xBFFF if self.ram_enabled => self.rtc.write(self.ram_rtc_bank, value),
            0xA000..=0xBFFF => (),
            _ => return false,
        }
        true
    }
}

#[typetag::serde]
impl MemoryBankController for MBC3 {
    fn start(&mut self) {
        let total_secs = self.rtc.seconds as u64
            + self.rtc.minutes as u64 * 60
            + self.rtc.hours as u64 * 3600
            + self.rtc.days as u64 * 24 * 3600;

        self.rtc.additional_secs = SystemTime::now()
            .duration_since(self.rtc.timestamp)
            .unwrap()
            .as_secs()
            + total_secs;
    }

    fn save(&mut self) {
        self.rtc.timestamp = SystemTime::now();
    }
}
