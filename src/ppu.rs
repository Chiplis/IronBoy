use std::cmp::max;
use std::ops::{Index, IndexMut, Range, RangeInclusive};

use crate::memory_map::{MemoryMap, MemoryRegion};
use crate::ppu::PpuState::{HBlank, OamSearch, PixelTransfer, VBlank};

enum PpuRegisterId { LcdControl, LcdStatus, LcdInterrupt, ScrollY, ScrollX, ScanLine, Background }

enum PpuRegisterAccess { R, W, RW }

type PpuRegisterAddress = u16;

struct PpuRegister(PpuRegisterAddress, u8, PpuRegisterId, PpuRegisterAccess);

#[derive(PartialEq, Copy, Clone)]
pub enum PpuState {
    OamSearch,
    PixelTransfer,
    HBlank,
    VBlank,
}

pub struct PPU {
    pixels_processed: u16,
    state: PpuState,
    tile_block_a: [u8; 0x8800 - 0x8000],
    tile_block_b: [u8; 0x9000 - 0x8800],
    tile_block_c: [u8; 0x9800 - 0x9000],
    tile_map_a: [u8; 0x9C00 - 0x9800],
    tile_map_b: [u8; 0xA000 - 0x9C00],
    oam: [u8; 0xFEA0 - 0xFE00],
    registers: [u8; 0xFF4C - 0xFF40],
    tick_count: u8,
    invalid: [u8; 1],
    ticks: u16,
}

#[derive(Copy, Clone)]
pub enum RenderResult {
    StateChange(PpuState, PpuState),
    ProcessingState(PpuState),
    Off,
}


impl PPU {
    pub fn new() -> Self {
        PPU {
            pixels_processed: 0,
            state: PpuState::OamSearch,
            tile_block_a: [0; 2048],
            tile_block_b: [0; 2048],
            tile_block_c: [0; 2048],
            tile_map_a: [0; 1024],
            tile_map_b: [0; 1024],
            oam: [0; 160],
            registers: [0; 12],
            tick_count: 0,
            invalid: [0xFF; 1],
            ticks: 0,
        }
    }

    pub fn line(&mut self) -> &mut u8 { self.read_mut(0xFF44) }

    pub fn lcdc(&mut self) -> &mut u8 { self.read_mut(0xFF40) }

    pub fn render_cycle(&mut self, cpu_cycles: u8) -> RenderResult {

        if *self.lcdc() & 0x80 == 0 {
            *self.line() = 0;
            self.ticks = 0;
            self.state = OamSearch;
            return RenderResult::Off
        }

        let old_state = self.state;

        self.ticks += (cpu_cycles as u16 * 4);
        self.ticks -= match self.state {
            PpuState::OamSearch => if self.ticks < 80 { 0 } else {
                self.state = PixelTransfer;
                80
            }

            PpuState::PixelTransfer => if self.ticks < 172 { 0 } else {
                self.state = HBlank;
                172
            }

            PpuState::HBlank => if self.ticks < 204 { 0 } else {
                *self.line() += 1;
                self.state = if *self.line() < 144 { OamSearch } else { VBlank };
                204
            }

            PpuState::VBlank => if self.ticks < 204 + 172 + 80 { 0 } else {
                *self.line() = (*self.line() + 1) % 154;
                self.state = if *self.line() == 0 { OamSearch } else { VBlank };
                204 + 172 + 80
            }
        };

        if old_state == self.state {
            RenderResult::ProcessingState(self.state)
        } else {
            RenderResult::StateChange(old_state, self.state)
        }
    }
}


#[deny(unreachable_patterns)]
impl MemoryRegion for PPU {
    fn sub_regions(&self) -> Vec<RangeInclusive<u16>> {
        vec![
            (0x8000..=0x9FFF),
            (0xFE00..=0xFE9F),
            (0xFF40..=0xFF4B)
        ]
    }

    fn read(&self, address: u16) -> &u8 {
        match address {
            (0x8000..=0x87FF) => &self.tile_block_a[(address - 0x8000) as usize],
            (0x8800..=0x8FFF) => &self.tile_block_b[(address - 0x8800) as usize],
            (0x9000..=0x97FF) => &self.tile_block_c[(address - 0x9000) as usize],
            (0x9800..=0x9BFF) => &self.tile_map_a[(address - 0x9800) as usize],
            (0x9C00..=0x9FFF) => &self.tile_map_b[(address - 0x9C00) as usize],

            (0xFE00..=0xFE9F) => if self.state == OamSearch || self.state == PixelTransfer {
                &self.invalid[0]
            } else { &self.oam[(address - 0xFE00) as usize] },

            (0xFF40..=0xFF4B) => &self.registers[(address - 0xFF40) as usize],
            _ => panic!()
        }
    }

    fn read_mut(&mut self, address: u16) -> &mut u8 {
        match address {
            (0x8000..=0x87FF) => &mut self.tile_block_a[(address - 0x8000) as usize],
            (0x8800..=0x8FFF) => &mut self.tile_block_b[(address - 0x8800) as usize],
            (0x9000..=0x97FF) => &mut self.tile_block_c[(address - 0x9000) as usize],
            (0x9800..=0x9BFF) => &mut self.tile_map_a[(address - 0x9800) as usize],
            (0x9C00..=0x9FFF) => &mut self.tile_map_b[(address - 0x9C00) as usize],

            (0xFE00..=0xFE9F) => if self.state == OamSearch || self.state == PixelTransfer {
                self.invalid[0] = 0x99;
                &mut self.invalid[0]
            } else { &mut self.oam[(address - 0xFE00) as usize] },

            (0xFF40..=0xFF4B) => &mut self.registers[(address - 0xFF40) as usize],

            _ => panic!()
        }
    }
}