use crate::cartridge::Cartridge;
use crate::mbc::MemoryBankController;
use crate::mmu::MemoryArea;
use std::cmp::max;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MBC5 {
    cartridge: Cartridge,
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u16,
    ram_bank: u8,
    rom_offset: usize,
    ram_offset: usize,
    ram_enabled: bool,
    expansion_mode: u8,
}

impl MBC5 {
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

impl MemoryBankController for MBC5 {}

impl MemoryArea for MBC5 {
    fn read(&self, address: usize) -> Option<u8> {
        Some(match address {
            0x0000..=0x3FFF => self.rom[address],
            0x4000..=0x7FFF => self.rom[self.rom_offset + (address & 0x3FFF)],
            0xA000..=0xBFFF if self.ram_enabled => self.ram[self.ram_offset + (address & 0x1FFF)],
            0xA000..=0xBFFF => 0xFF,
            _ => return None,
        })
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x0000..=0x1FFF => match self.cartridge.mbc {
                2 | 3 => self.ram_enabled = value & 0x0F == 0x0A,
                _ => (),
            },
            0x2000..=0x3FFF => match self.cartridge.mbc {
                1 | 2 | 3 => {
                    self.rom_bank = (self.rom_bank & 0x60) + max(1, value as u16 & 0x1FF);
                    self.rom_offset = self.rom_bank as usize * 0x4000;
                }
                _ => (),
            },
            0x4000..=0x5FFF => match self.cartridge.mbc {
                1 | 2 | 3 if self.expansion_mode != 0 => {
                    self.ram_bank = value & 3;
                    self.ram_offset = self.ram_bank as usize * 0x2000;
                }
                1 | 2 | 3 => {
                    self.rom_bank = (self.rom_bank & 0x1FF) + ((value as u16 & 3) << 5);
                    self.rom_offset = self.rom_bank as usize * 0x4000;
                }
                _ => (),
            },
            0x6000..=0x7FFF => match self.cartridge.mbc {
                2 | 3 => self.expansion_mode = value & 1,
                _ => (),
            },
            0xA000..=0xBFFF if self.ram_enabled => {
                self.ram[self.ram_offset + (address & 0x1FFF)] = value
            }
            0xA000..=0xBFFF => (),
            _ => return false,
        }
        true
    }
}
