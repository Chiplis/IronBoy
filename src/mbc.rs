use crate::mmu::MemoryArea;

#[typetag::serde(tag = "type")]
pub trait MemoryBankController: MemoryArea {}
