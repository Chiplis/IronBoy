use std::cmp::max;
use std::ops::{Index, IndexMut, Range, RangeInclusive};

use crate::memory_map::{MemoryMap};
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
    invalid: u8,
    ticks: u16,
    off: bool,
}

#[derive(Copy, Clone)]
pub enum RenderResult {
    StateChange(PpuState, PpuState),
    ProcessingState(PpuState),
    LcdOff,
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
            invalid: 0xFF,
            ticks: 0,
            off: false,
        }
    }

    pub fn line(&mut self) -> &mut u8 { &mut self.registers[4] }

    pub fn lcdc(&mut self) -> &u8 { self.read(0xFF40).unwrap() }

    pub fn render_cycle(&mut self, cpu_cycles: u8) -> RenderResult {

        if *self.lcdc() & 0x80 == 0 {
            *self.line() = 0;
            self.ticks = 0;
            self.state = OamSearch;
            return RenderResult::LcdOff
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
                self.state = if *self.line() == 144 { VBlank } else { OamSearch };
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

    pub fn read(&self, address: usize) -> Option<&u8> {
        match (address, self.state) {
            (0x8000..=0x87FF, _) => Some(&self.tile_block_a[(address - 0x8000) as usize]),
            (0x8800..=0x8FFF, _) => Some(&self.tile_block_b[(address - 0x8800) as usize]),
            (0x9000..=0x97FF, _) => Some(&self.tile_block_c[(address - 0x9000) as usize]),
            (0x9800..=0x9BFF, _) => Some(&self.tile_map_a[(address - 0x9800) as usize]),
            (0x9C00..=0x9FFF, _) => Some(&self.tile_map_b[(address - 0x9C00) as usize]),

            (0xFE00..=0xFE9F, OamSearch) | (0xFE00..=0xFE9F, PixelTransfer) => Some(&0xFF),
            (0xFE00..=0xFE9F, _) => Some(&self.oam[(address - 0xFE00) as usize]),
            (0xFF40..=0xFF4B, _) => Some(&self.registers[(address - 0xFF40) as usize]),
            _ => None
        }
    }

    pub(crate) fn write(&mut self, address: usize, value: u8) -> bool {
        let mut wrote = true;
        match (address, self.state) {
            (0x8000..=0x87FF, _) => self.tile_block_a[address - 0x8000] = value,
            (0x8800..=0x8FFF, _) => self.tile_block_b[address - 0x8800] = value,
            (0x9000..=0x97FF, _) => self.tile_block_c[address - 0x9000] = value,
            (0x9800..=0x9BFF, _) => self.tile_map_a[address - 0x9800] = value,
            (0x9C00..=0x9FFF, _) => self.tile_map_b[address - 0x9C00] = value,

            (0xFF44, _) | (0xFE00..=0xFE9F, OamSearch) | (0xFE00..=0xFE9F, PixelTransfer) => {},
            (0xFE00..=0xFE9F, _) => self.oam[address - 0xFE00] = value,
            (0xFF40..=0xFF43, _) | (0xFF45..=0xFF4B, _) => self.registers[address - 0xFF40] = value,
            _ => { wrote = false; }
        }
        wrote
    }
}