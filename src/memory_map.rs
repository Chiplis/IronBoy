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
    pub fn new() -> Self { MemoryMap { memory: [0; 0x10000] } }

    pub fn write_echo_byte(&mut self, addr: u16, content: u8) {
        let should_echo = (0xC000 >= addr && addr <= 0xDDFF) || (0xE000 >= addr && addr <= 0xFDFF);
        self.memory[addr as usize] = content;
        if should_echo {
            let offset = if addr < 0xE000 { 0x2000 } else { -0x2000 };
            let echo_addr = (addr as i16 + offset) as u16;
            self.memory[echo_addr as usize] = content;
        }
    }
}

#[test]
fn test_echo_rw() {
    let mem = &mut MemoryMap::new();
    let content = 0x42;
    let address = 0xC000;
    let echo_address = 0xE000;

    mem.write_echo_byte(address, content);
    assert_eq!(mem[address], mem[echo_address]);
    mem.write_echo_byte(echo_address, content + 1);
    assert_ne!(mem[address], content);
    assert_eq!(mem[address], mem[echo_address]);
}