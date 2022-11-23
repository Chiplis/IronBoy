use std::str::from_utf8;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, Eq, PartialEq, PartialOrd)]
pub struct Cartridge {
    pub(crate) title: Option<String>,
    publisher: Option<String>,
    pub(crate) mbc: u8,
    rom_size: usize,
    ram_size: u8,
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
        Self {
            title: from_utf8(title.as_slice()).map(|t| t.to_string()).ok(),
            publisher: from_utf8(&rom[0x144..=0x145]).map(|t| t.to_string()).ok(),
            mbc: rom[0x147],
            rom_size: 32 << rom[0x148],
            ram_size: rom[0x149],
            destination: rom[0x14A],
            old_publisher: rom[0x14B],
            rom_version: rom[0x14C],
            header_checksum: rom[0x14D],
            global_checksum: u16::from_be_bytes([rom[0x14E], rom[0x14F]]),
        }
    }
}
