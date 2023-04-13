use crate::mmu::MemoryArea;

pub trait MemoryBankController: MemoryArea {
    fn start(&mut self) {}

    fn save(&mut self) {}
}
