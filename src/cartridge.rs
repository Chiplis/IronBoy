use std::str::from_utf8;
use crate::mmu::MemoryManagementUnit;

#[derive(Debug)]
pub struct Cartridge {
    title: String,
    publisher: String,
    mbc: u8,
    rom_size: usize,
    ram_size: u8,
    destination: u8,
    old_publisher: u8,
    rom_version: u8,
    header_checksum: u8,
    global_checksum: u16
}

impl Cartridge {
    pub fn new(rom: &Vec<u8>) -> Self {
        Self {
            title: from_utf8(&rom[0x134..=0x143]).unwrap().to_string(),
            publisher: from_utf8(&rom[0x144..=0x145]).unwrap().to_string(),
            mbc: rom[0x147],
            rom_size: 32 << rom[0x148],
            ram_size: rom[0x149],
            destination: rom[0x14A],
            old_publisher: rom[0x14B],
            rom_version: rom[0x14C],
            header_checksum: rom[0x14D],
            global_checksum: u16::from_be_bytes([rom[0x14E], rom[0x14F]])
        }
    }
}