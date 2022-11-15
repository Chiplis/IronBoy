use crate::cartridge::Cartridge;
use crate::mmu::{MemoryArea};

pub trait MemoryBankController: MemoryArea {}