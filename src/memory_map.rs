use std::convert::TryInto;
use std::fmt::Display;
use std::ops::{Index, IndexMut};

use crate::interrupt::{InterruptId, Interrupt};
use crate::interrupt::InterruptId::{Joypad, Serial, STAT, Timer, VBlank};
use crate::ppu::{MemoryRegion, PPU, PpuState, RenderResult};
use crate::register::{ByteRegister, WordRegister};
use std::slice::Iter;

pub struct MemoryMap {
    memory: [u8; 0x10000],
    pub(crate) interrupt: Interrupt,
    ppu: PPU
}

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

impl MemoryMap {

    pub fn new(rom: Vec<u8>) -> Self {
        let ppu = PPU::new();
        let interrupt = Interrupt::new();
        let mut mem = MemoryMap {
            ppu,
            interrupt,
            memory: MemoryMap::init_memory()
        };
        rom.iter().enumerate().for_each(|(index, v)| mem.memory[index] = *v);
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
        } else {
            &mut self.memory[address.into()]
        }
    }

    pub fn cycle(&mut self, cpu_cycles: u8) {
        match self.ppu.render_cycle(cpu_cycles) {
            RenderResult::StateChange(_, PpuState::VBlank) => self.interrupt.set(VBlank, true),
            _ => { }
        }
    }

    pub fn init_memory() -> [u8; 0x10000]{
        let mut mem = [0; 0x10000];
        mem[0xFF05_usize] = 0;
        mem[0xFF06_usize] = 0;
        mem[0xFF07_usize] = 0;
        mem[0xFF10_usize] = 0x80;
        mem[0xFF11_usize] = 0xBF;
        mem[0xFF12_usize] = 0xF3;
        mem[0xFF14_usize] = 0xBF;
        mem[0xFF16_usize] = 0x3F;
        mem[0xFF16_usize] = 0x3F;
        mem[0xFF17_usize] = 0;
        mem[0xFF19_usize] = 0xBF;
        mem[0xFF1A_usize] = 0x7F;
        mem[0xFF1B_usize] = 0xFF;
        mem[0xFF1C_usize] = 0x9F;
        mem[0xFF1E_usize] = 0xFF;
        mem[0xFF20_usize] = 0xFF;
        mem[0xFF21_usize] = 0;
        mem[0xFF22_usize] = 0;
        mem[0xFF23_usize] = 0xBF;
        mem[0xFF24_usize] = 0x77;
        mem[0xFF25_usize] = 0xF3;
        mem[0xFF26_usize] = 0xF1;
        mem[0xFF40_usize] = 0x91;
        mem[0xFF42_usize] = 0;
        mem[0xFF43_usize] = 0;
        mem[0xFF45_usize] = 0;
        mem[0xFF47_usize] = 0xFC;
        mem[0xFF48_usize] = 0xFF;
        mem[0xFF49_usize] = 0xFF;
        mem[0xFF4A_usize] = 0;
        mem[0xFF4B_usize] = 0;
        mem[0xFF00_usize] = 0xFF;
        mem
    }
}
