use std::str::from_utf8;

use serde::{Deserialize, Serialize};
use crate::logger::Logger;

#[derive(Serialize, Deserialize, Default, Debug, Eq, PartialEq, PartialOrd)]
pub struct Cartridge {
    pub(crate) title: Option<String>,
    publisher: Option<String>,
    pub(crate) mbc: u8,
    pub(crate) rom_size: usize,
    pub(crate) rom_bank_count: u16,
    pub(crate) ram_bank_count: u8,
    pub(crate) ram_size: u8,
    destination: u8,
    old_publisher: u8,
    rom_version: u8,
    header_checksum: u8,
    global_checksum: u16,
}

impl Cartridge {
    pub fn new(rom: &[u8]) -> Self {
        let title: Vec<u8> = rom[0x134..=0x143]
            .iter()
            .copied()
            .take_while(|c| *c != 0)
            .collect();
        let s = Self {
            title: from_utf8(title.as_slice()).map(|t| t.to_string()).ok(),
            publisher: from_utf8(&rom[0x144..=0x145]).map(|t| t.to_string()).ok(),
            mbc: rom[0x147],
            rom_size: 32 << rom[0x148],
            rom_bank_count: 2_u16.pow(rom[0x148] as u32 + 1) as u16,
            ram_bank_count: match rom[0x149] {
                0x00 => 0,
                0x02 => 1,
                0x03 => 4,
                0x04 => 16,
                0x05 => 8,
                _ => unreachable!()
            },
            ram_size: rom[0x149],
            destination: rom[0x14A],
            old_publisher: rom[0x14B],
            rom_version: rom[0x14C],
            header_checksum: rom[0x14D],
            global_checksum: u16::from_be_bytes([rom[0x14E], rom[0x14F]]),
        };
        Logger::info(format!("Cartridge: {s:?}"));
        s
    }
}
