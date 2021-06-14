use std::convert::TryInto;
use std::fmt::Display;
use std::ops::{Index, IndexMut, RangeInclusive};
use std::slice::Iter;

use crate::interrupt::{Interrupt, InterruptId};
use crate::interrupt::InterruptId::{Joypad, Serial, STAT, Timer, VBlank};
use crate::ppu::{PPU, PpuState, RenderResult};
use crate::ppu::PpuState::{OamSearch, PixelTransfer};
use crate::register::{ByteRegister, WordRegister};

impl Index<u16> for MemoryMap {
    type Output = u8;
    fn index(&self, index: u16) -> &Self::Output { self.get(index) }
}

impl IndexMut<u16> for MemoryMap {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output { self.get_mut(index) }
}

impl Index<WordRegister> for MemoryMap {
    type Output = u8;
    fn index(&self, index: WordRegister) -> &Self::Output { self.get(index.value()) }
}

impl IndexMut<WordRegister> for MemoryMap {
    fn index_mut(&mut self, index: WordRegister) -> &mut Self::Output { self.get_mut(index.value()) }
}

impl Index<ByteRegister> for MemoryMap {
    type Output = u8;
    fn index(&self, index: ByteRegister) -> &Self::Output { self.get(index.0 as u16 + 0xFF00) }
}

impl IndexMut<ByteRegister> for MemoryMap {
    fn index_mut(&mut self, index: ByteRegister) -> &mut Self::Output { self.get_mut(index.0 as u16 + 0xFF00) }
}

impl Index<u8> for MemoryMap {
    type Output = u8;
    fn index(&self, index: u8) -> &Self::Output { self.get(0xFF00 + index as u16) }
}

impl IndexMut<u8> for MemoryMap {
    fn index_mut(&mut self, index: u8) -> &mut Self::Output { self.get_mut(0xFF00 + index as u16) }
}

pub struct MemoryMap {
    memory: [u8; 0x10000],
    pub(crate) interrupt: Interrupt,
    ppu: PPU,
    invalid: u8,
    rom_size: usize,
}

impl MemoryMap {
    pub fn new(rom: Vec<u8>) -> Self {
        let ppu = PPU::new();
        let interrupt = Interrupt::new();
        let mut mem = MemoryMap {
            ppu,
            interrupt,
            memory: [0; 0x10000],
            rom_size: rom.len() as usize,
            invalid: 0xFF,
        };
        mem.init_memory();
        rom.iter().enumerate().for_each(|(index, v)| mem[index as u16] = *v);
        mem
    }

    fn get<T: Into<usize> + Display + Copy>(&self, address: T) -> &u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory[address.into()]);
        if self.ppu.sub_regions().iter().any(|sr| sr.contains(&(address.into() as u16))) {
            self.ppu.read(address.into() as u16)
        } else if self.interrupt.sub_regions().iter().any(|sr| sr.contains(&(address.into() as u16))) {
            self.interrupt.read(address.into() as u16)
        } else {
            &self.memory[address.into()]
        }
    }

    fn get_mut<T: Into<usize> + Display + Copy>(&mut self, address: T) -> &mut u8 {
        //println!("Writing address {}", address.into());
        if self.ppu.sub_regions().iter().any(|sr| sr.contains(&(address.into() as u16))) {
            self.ppu.read_mut(address.into() as u16)
        } else if self.interrupt.sub_regions().iter().any(|sr| sr.contains(&(address.into() as u16))) {
            self.interrupt.read_mut(address.into() as u16)
        } else if address.into() < self.rom_size {
            return &mut self.invalid;
        } else {
            &mut self.memory[address.into()]
        }
    }

    pub fn cycle(&mut self, cpu_cycles: u8) {
        match self.ppu.render_cycle(cpu_cycles) {
            RenderResult::StateChange(_, PpuState::VBlank) => self.interrupt.set(VBlank, true),
            _ => {}
        }
    }

    fn init_memory(&mut self) {
        self[0xFF05_u16] = 0;
        self[0xFF06_u16] = 0;
        self[0xFF07_u16] = 0;
        self[0xFF10_u16] = 0x80;
        self[0xFF11_u16] = 0xBF;
        self[0xFF12_u16] = 0xF3;
        self[0xFF14_u16] = 0xBF;
        self[0xFF16_u16] = 0x3F;
        self[0xFF16_u16] = 0x3F;
        self[0xFF17_u16] = 0;
        self[0xFF19_u16] = 0xBF;
        self[0xFF1A_u16] = 0x7F;
        self[0xFF1B_u16] = 0xFF;
        self[0xFF1C_u16] = 0x9F;
        self[0xFF1E_u16] = 0xFF;
        self[0xFF20_u16] = 0xFF;
        self[0xFF21_u16] = 0;
        self[0xFF22_u16] = 0;
        self[0xFF23_u16] = 0xBF;
        self[0xFF24_u16] = 0x77;
        self[0xFF25_u16] = 0xF3;
        self[0xFF26_u16] = 0xF1;
        self[0xFF40_u16] = 0x91;
        self[0xFF42_u16] = 0;
        self[0xFF43_u16] = 0;
        self[0xFF45_u16] = 0;
        self[0xFF47_u16] = 0xFC;
        self[0xFF48_u16] = 0xFF;
        self[0xFF49_u16] = 0xFF;
        self[0xFF4A_u16] = 0;
        self[0xFF4B_u16] = 0;
        self[0xFF00_u16] = 0xFF;
    }
}

pub trait MemoryRegion {
    fn sub_regions(&self) -> Vec<RangeInclusive<u16>>;
    fn read(&self, address: u16) -> &u8;
    fn read_mut(&mut self, address: u16) -> &mut u8;
}
