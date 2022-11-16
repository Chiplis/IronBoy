use std::cmp::max;
use crate::cartridge::Cartridge;
use crate::mbc::MemoryBankController;
use crate::mmu::MemoryArea;

#[derive(Default)]
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
    rtc: [u8; 5],
    rtc_enabled: bool,
}

impl MBC3 {
    pub fn new(cartridge: Cartridge, rom: Vec<u8>) -> Self {
        Self {
            cartridge,
            rom,
            ram: vec![0; 1024 * 1024 * 2],
            rom_offset: 0x4000,
            ..Default::default()
        }
    }
}

impl MemoryArea for MBC3 {
    fn read(&self, address: usize) -> Option<u8> {
        Some(match address {
            0x0000..=0x3FFF => self.rom[address],
            0x4000..=0x7FFF => self.rom[self.rom_offset + (address & 0x3FFF)],
            0xA000..=0xBFFF if self.ram_enabled && !self.rtc_enabled => self.ram[self.ram_offset + (address & 0x1FFF)],
            0xA000..=0xBFFF if self.ram_enabled => 0x00,
            0xA000..=0xBFFF => 0xFF,
            _ => return None,
        })
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A
            }
            0x2000..=0x3FFF => {
                self.rom_bank = max(1, value) & 0x7F;
                self.rom_offset = self.rom_bank as usize * 0x4000;
            }
            0x4000..=0x5FFF => {
                if self.expansion_mode != 0 {
                    self.ram_rtc_bank = if value >= 0x08 {
                        value & 0x0C
                    } else {
                        value & 3
                    };
                    self.rtc_enabled = self.ram_rtc_bank > 0x03;
                    self.ram_offset = self.ram_rtc_bank as usize * 0x2000;
                } else {
                    self.rom_bank = (self.rom_bank & 0x1F) + ((value & 3) << 5);
                    self.rom_offset = self.rom_bank as usize * 0x4000;
                }
            }
            0x6000..=0x7FFF => {
                if !self.rtc_enabled {
                    self.expansion_mode = value & 1
                }
            }
            0xA000..=0xBFFF if self.ram_enabled && !self.rtc_enabled => {
                self.ram[self.ram_offset + (address & 0x1FFF)] = value
            }
            0xA000..=0xBFFF => (),
            _ => return false,
        }
        true
    }
}

impl MemoryBankController for MBC3 {}