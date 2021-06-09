use std::ops::{Index, IndexMut};
use crate::register::{SpecialRegister, ByteRegister};

#[derive(Clone)]
pub struct MemoryMap {
    memory: [u8; 0x10000],
}

impl Index<u8> for MemoryMap {
    type Output = u8;
    fn index(&self, index: u8) -> &Self::Output { &self.memory[0xFF00 + index as usize] }
}

impl IndexMut<u8> for MemoryMap {
    fn index_mut(&mut self, index: u8) -> &mut Self::Output { &mut self.memory[0xFF00 + index as usize] }
}

impl Index<u16> for MemoryMap {
    type Output = u8;
    fn index(&self, index: u16) -> &Self::Output { &self.memory[index as usize] }
}

impl IndexMut<u16> for MemoryMap {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output { &mut self.memory[index as usize] }
}

impl Index<ByteRegister> for MemoryMap {
    type Output = u8;
    fn index(&self, index: ByteRegister) -> &Self::Output { &self.memory[0xFF00 + index.0 as usize] }
}

impl IndexMut<ByteRegister> for MemoryMap {
    fn index_mut(&mut self, index: ByteRegister) -> &mut Self::Output { &mut self.memory[0xFF00 + index.0 as usize] }
}

impl Index<SpecialRegister> for MemoryMap {
    type Output = u8;
    fn index(&self, index: SpecialRegister) -> &Self::Output { &self.memory[index.value() as usize] }
}

impl IndexMut<SpecialRegister> for MemoryMap {
    fn index_mut(&mut self, index: SpecialRegister) -> &mut Self::Output { &mut self.memory[index.value() as usize] }
}

impl MemoryMap {
    pub fn new() -> Self {
        let mut mem: [u8; 0x10000] = [0; 0x10000];
        mem[0xFF05] = 0;
        mem[0xFF06] = 0;
        mem[0xFF07] = 0;
        mem[0xFF10] = 0x80;
        mem[0xFF11] = 0xBF;
        mem[0xFF12] = 0xF3;
        mem[0xFF14] = 0xBF;
        mem[0xFF16] = 0x3F;
        mem[0xFF16] = 0x3F;
        mem[0xFF17] = 0;
        mem[0xFF19] = 0xBF;
        mem[0xFF1A] = 0x7F;
        mem[0xFF1B] = 0xFF;
        mem[0xFF1C] = 0x9F;
        mem[0xFF1E] = 0xFF;
        mem[0xFF20] = 0xFF;
        mem[0xFF21] = 0;
        mem[0xFF22] = 0;
        mem[0xFF23] = 0xBF;
        mem[0xFF24] = 0x77;
        mem[0xFF25] = 0xF3;
        mem[0xFF26] = 0xF1;
        mem[0xFF40] = 0x91;
        mem[0xFF42] = 0;
        mem[0xFF43] = 0;
        mem[0xFF45] = 0;
        mem[0xFF47] = 0xFC;
        mem[0xFF48] = 0xFF;
        mem[0xFF49] = 0xFF;
        mem[0xFF4A] = 0;
        mem[0xFF4B] = 0;

        MemoryMap { memory: mem }
    }

    pub fn write_register(&mut self, register: ByteRegister, content: u8) {
        self.write_offset(register.0, content);
    }

    pub fn write_offset(&mut self, offset: u8, content: u8) {
        self.write_byte(offset as u16 + 0xFF00, content);
    }

    pub fn write_byte(&mut self, addr: u16, content: u8) {
        self.memory[addr as usize] = content;
        let should_echo = (0xC000 <= addr && addr <= 0xDDFF) || (0xE000 <= addr && addr <= 0xFDFF);
        if should_echo {
            let offset = if addr < 0xE000 { 0x2000 } else { -0x2000 };
            let echo_addr = (addr as i32 + offset) as u16;
            self.memory[echo_addr as usize] = content;
        }
    }
}

#[test]
fn test_echo_rw() {
    let mem = &mut MemoryMap::new();
    let content = 0x42;
    let address = 0xDDFF;
    let echo_address = 0xFDFF;

    mem.write_byte(address, content);
    assert_eq!(mem[address], mem[echo_address]);
    mem.write_byte(echo_address, content + 1);
    assert_ne!(mem[address], content);
    assert_eq!(mem[address], mem[echo_address]);
}