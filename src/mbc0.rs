use crate::mbc::MemoryBankController;
use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MBC0 {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
}

impl MemoryBankController for MBC0 {}

impl MBC0 {
    pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
        Self { rom, ram }
    }
}

impl MemoryArea for MBC0 {
    fn read(&self, address: usize) -> Option<u8> {
        Some(match address {
            0x0000..=0x7FFF => self.rom[address],
            0xA000..=0xBFFF => self.ram[address - 0xA000],
            _ => return None,
        })
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x0000..=0x7FFF => return true,
            0xA000..=0xBFFF => self.ram[address - 0xA000] = value,
            _ => return false,
        }
        true
    }
}
