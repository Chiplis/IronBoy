use crate::cartridge::Cartridge;
use crate::mbc::MemoryBankController;
use crate::mmu::MemoryArea;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MBC1 {
    cartridge: Cartridge,
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_bank_low: u8,
    bank_high: u8,
    ram_enabled: bool,
    banking_mode: u8,
    multicart: bool,
}

impl MBC1 {
    pub fn new(cartridge: Cartridge, rom: Vec<u8>) -> Self {
        let ram_size = cartridge.ram_bank_count as usize * 0x2000;
        let multicart = Self::is_multicart(&rom);
        Self {
            cartridge,
            rom,
            ram: vec![0; ram_size],
            rom_bank_low: 1,
            bank_high: 0,
            ram_enabled: false,
            banking_mode: 0,
            multicart,
        }
    }

    fn is_multicart(rom: &[u8]) -> bool {
        // MBC1M multicarts have a second valid cartridge header at bank $10.
        // A repeated Nintendo logo is the conventional emulator heuristic,
        // since the primary cartridge header cannot distinguish MBC1M.
        const SUB_HEADER: usize = 0x10 * 0x4000;
        if rom.len() < SUB_HEADER + 0x150 {
            return false;
        }

        rom[0x104..=0x133] == rom[SUB_HEADER + 0x104..=SUB_HEADER + 0x133]
    }

    fn rom_bank_count(&self) -> usize {
        usize::from(self.cartridge.rom_bank_count).max(1)
    }

    fn selected_upper_rom_bank(&self) -> usize {
        let low_mask = if self.multicart { 0x0F } else { 0x1F };
        let high_shift = if self.multicart { 4 } else { 5 };
        let low = if self.rom_bank_low == 0 {
            1
        } else {
            usize::from(self.rom_bank_low & low_mask)
        };
        ((usize::from(self.bank_high) << high_shift) | low) % self.rom_bank_count()
    }

    fn selected_lower_rom_bank(&self) -> usize {
        if self.banking_mode == 0 {
            return 0;
        }
        let high_shift = if self.multicart { 4 } else { 5 };
        (usize::from(self.bank_high) << high_shift) % self.rom_bank_count()
    }

    fn selected_ram_bank(&self) -> usize {
        if self.banking_mode == 0 || self.cartridge.ram_bank_count == 0 {
            0
        } else {
            usize::from(self.bank_high) % usize::from(self.cartridge.ram_bank_count)
        }
    }
}

impl MemoryBankController for MBC1 {}

impl MemoryArea for MBC1 {
    fn read(&self, address: usize) -> Option<u8> {
        Some(match address {
            0x0000..=0x3FFF => {
                let offset = self.selected_lower_rom_bank() * 0x4000 + address;
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            0x4000..=0x7FFF => {
                let offset = self.selected_upper_rom_bank() * 0x4000 + (address & 0x3FFF);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF if self.ram_enabled && !self.ram.is_empty() => {
                let offset = self.selected_ram_bank() * 0x2000 + (address & 0x1FFF);
                self.ram.get(offset).copied().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => 0xFF,
            _ => return None,
        })
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x0000..=0x1FFF => self.ram_enabled = value & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank_low = value & 0x1F,
            0x4000..=0x5FFF => self.bank_high = value & 0x03,
            0x6000..=0x7FFF => self.banking_mode = value & 0x01,
            0xA000..=0xBFFF if self.ram_enabled && !self.ram.is_empty() => {
                let offset = self.selected_ram_bank() * 0x2000 + (address & 0x1FFF);
                if let Some(byte) = self.ram.get_mut(offset) {
                    *byte = value;
                }
            }
            0xA000..=0xBFFF => (),
            _ => return false,
        }
        true
    }
}
