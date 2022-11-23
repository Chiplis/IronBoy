use crate::mmu::MemoryArea;

#[typetag::serde(tag = "type")]
pub trait MemoryBankController: MemoryArea {
    fn start(&mut self) {}

    fn save(&mut self) {}
}
