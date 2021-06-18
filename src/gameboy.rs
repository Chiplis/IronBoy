use std::ops::{Index, IndexMut};

use crate::memory_map::MemoryMap;
use crate::register::{ByteRegister, ConditionCode, FlagRegister, ProgramCounter, RegisterId, WordRegister, Register};
use crate::register::RegisterId::{A, B, C, D, E, H, L};
use crate::register::WordRegister::StackPointer;

pub struct Gameboy {
    pub registers: Register,
    pub vram: [u8; 2 * 8 * 1024],
    pub ime_counter: i8,
    pub ime: bool,
    pub mem: MemoryMap,
    pub(crate) halted: bool
}

impl Gameboy {
    pub fn new(mem: MemoryMap) -> Self {
        Self {
            registers: Register::new(),
            mem,
            vram: [0; 2 * 8 * 1024],
            ime_counter: -1,
            ime: false,
            halted: false
        }
    }
}

impl Gameboy {}

impl Index<RegisterId> for Gameboy {
    type Output = ByteRegister;

    fn index(&self, index: RegisterId) -> &Self::Output {
        &self.registers[index]
    }
}

impl IndexMut<RegisterId> for Gameboy {
    fn index_mut(&mut self, index: RegisterId) -> &mut Self::Output {
        &mut self.registers[index]
    }
}
