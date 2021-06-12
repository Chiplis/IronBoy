use std::ops::{Index, IndexMut};
use crate::register::{WordRegister, ByteRegister};
use crate::memory_map::GpuRegisterId::LcdControl;
use std::fmt::Display;
use crate::instruction::{Interrupt, InterruptId};
use crate::instruction::InterruptId::{VBlank, Joypad, Serial, Timer, STAT};
use std::convert::TryInto;

#[derive(Clone)]
pub struct MemoryMap {
    memory: [u8; 0x10000],
    pub(crate) interrupts: [Interrupt; 5]
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

impl Index<InterruptId> for MemoryMap {
    type Output = Interrupt;
    fn index(&self, id: InterruptId) -> &Self::Output {
        self.interrupts.iter().find(|i| i.id == id).unwrap()
    }
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
    fn index(&self, index: u8) -> &Self::Output { &self.memory[0xFF00 + index as usize] }
}

impl IndexMut<u8> for MemoryMap {
    fn index_mut(&mut self, index: u8) -> &mut Self::Output { &mut self.memory[0xFF00 + index as usize] }
}

enum GpuRegisterId { LcdControl, LcdStatus, LcdInterrupt, ScrollY, ScrollX, ScanLine, Background }

enum GpuRegisterAccess { R, W, RW }

struct GpuRegister(u16, u8, GpuRegisterId, GpuRegisterAccess);

struct GPU {
    control: GpuRegister,
    scroll_y: GpuRegister,
    scroll_x: GpuRegister,
    scan_line: GpuRegister,
    background: GpuRegister,
}

impl Interrupt {
    const IE_ADDRESS: u16 = 0xFFFF;
    const IF_ADDRESS: u16 = 0xFF0F;

    pub fn handler(mem: &[u8; 0x10000]) -> [Interrupt; 5] {
        [
            Interrupt { mem: *mem, id: VBlank, mask: 0x01 },
            Interrupt { mem: *mem, id: STAT, mask: 0x02 },
            Interrupt { mem: *mem, id: Timer, mask: 0x04 },
            Interrupt { mem: *mem, id: Serial, mask: 0x08 },
            Interrupt { mem: *mem, id: Joypad, mask: 0x10 },
        ]
    }

    pub fn enabled(&self) -> bool {
        self.mem[Interrupt::IE_ADDRESS as usize] & self.mem[Interrupt::IF_ADDRESS as usize] & self.mask != 0
    }

}

impl MemoryMap {

    pub fn new(rom: Vec<u8>) -> Self {
        let mut memory: [u8; 0x10000] = [0; 0x10000];
        let mut mem = MemoryMap { memory, interrupts: Interrupt::handler(&memory) };
        mem[0xFF05_u16] = 0;
        mem[0xFF06_u16] = 0;
        mem[0xFF07_u16] = 0;
        mem[0xFF10_u16] = 0x80;
        mem[0xFF11_u16] = 0xBF;
        mem[0xFF12_u16] = 0xF3;
        mem[0xFF14_u16] = 0xBF;
        mem[0xFF16_u16] = 0x3F;
        mem[0xFF16_u16] = 0x3F;
        mem[0xFF17_u16] = 0;
        mem[0xFF19_u16] = 0xBF;
        mem[0xFF1A_u16] = 0x7F;
        mem[0xFF1B_u16] = 0xFF;
        mem[0xFF1C_u16] = 0x9F;
        mem[0xFF1E_u16] = 0xFF;
        mem[0xFF20_u16] = 0xFF;
        mem[0xFF21_u16] = 0;
        mem[0xFF22_u16] = 0;
        mem[0xFF23_u16] = 0xBF;
        mem[0xFF24_u16] = 0x77;
        mem[0xFF25_u16] = 0xF3;
        mem[0xFF26_u16] = 0xF1;
        mem[0xFF40_u16] = 0x91;
        mem[0xFF42_u16] = 0;
        mem[0xFF43_u16] = 0;
        mem[0xFF45_u16] = 0;
        mem[0xFF47_u16] = 0xFC;
        mem[0xFF48_u16] = 0xFF;
        mem[0xFF49_u16] = 0xFF;
        mem[0xFF4A_u16] = 0;
        mem[0xFF4B_u16] = 0;
        rom.iter().enumerate().for_each(|(index, v)| mem.memory[index] = *v);
        mem
    }

    fn get<T: Into<usize> + Display + Copy>(&self, address: T) -> &u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory[address.into()]);
        &self.memory[address.into()]
    }

    fn get_mut<T: Into<usize> + Display + Copy>(&mut self, address: T) -> &mut u8 {
        //println!("Writing address {}", address.into());
        &mut self.memory[address.into()]
    }

    pub fn set_interrupt(&mut self, interrupt: InterruptId, set: bool) {
        if set {
            self[Interrupt::IF_ADDRESS] |= self[interrupt].mask;
        } else {
            self[Interrupt::IF_ADDRESS] &= !self[interrupt].mask;
        }
    }
}
