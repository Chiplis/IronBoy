use std::cmp::max;
use crate::cartridge::Cartridge;
use crate::mbc::MemoryBankController;
use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MBC2 {
    cartridge: Cartridge,
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank: u8,
    rom_offset: usize,
    ram_enabled: bool,
}

impl MBC2 {
    pub fn new(cartridge: Cartridge, rom: Vec<u8>) -> Self {
        Self {
            cartridge,
            rom,
            ram: vec![0; 0x0200],
            rom_offset: 0x4000,
            ..Default::default()
        }
    }
}

impl MemoryBankController for MBC2 {}

impl MemoryArea for MBC2 {
    fn read(&self, address: usize) -> Option<u8> {
        Some(match address {
            0x0000..=0x3FFF => self.rom[address],
            0x4000..=0x7FFF => self.rom[self.rom_offset + (address & 0x3FFF)],
            0xA000..=0xA1FF if self.ram_enabled => self.ram[address & 0x01FF],
            0xA000..=0xA1FF => 0xFF,
            _ => return None,
        })
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x0000..=0x1FFF => {
                if (address & 0x0100) == 0 {
                    self.ram_enabled = value & 0x0F == 0x0A;
                }
            }
            0x2000..=0x3FFF => {
                if (address & 0x0100) != 0 {
                    self.rom_bank = max(1, value & 0x0F);
                    self.rom_offset = self.rom_bank as usize * 0x4000;
                }
            }
            0xA000..=0xA1FF if self.ram_enabled => self.ram[address & 0x01FF] = value & 0x0F,
            0xA000..=0xA1FF => (),
            _ => return false,
        }
        true
    }
}
