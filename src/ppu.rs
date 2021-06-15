use std::cmp::max;
use std::ops::{Index, IndexMut, Range, RangeInclusive};

use crate::memory_map::{MemoryMap};
use crate::ppu::PpuState::{HBlank, OamSearch, PixelTransfer, VBlank};
use crate::interrupt::{Interrupt, InterruptId};
use crate::ppu::StatInterrupt::{Low, ModeInt, LycInt};
use crate::ppu::RenderResult::LcdOff;

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
    mode: PpuState,
    tile_block_a: [u8; 0x8800 - 0x8000],
    tile_block_b: [u8; 0x9000 - 0x8800],
    tile_block_c: [u8; 0x9800 - 0x9000],
    tile_map_a: [u8; 0x9C00 - 0x9800],
    tile_map_b: [u8; 0xA000 - 0x9C00],
    oam: [u8; 0xFEA0 - 0xFE00],
    registers: [u8; 0xFF4C - 0xFF40],
    invalid: u8,
    ticks: u16,
    last_render: RenderResult,
    stat_line: StatInterrupt,
}

#[derive(PartialEq, Clone, Copy)]
pub enum RenderResult {
    StateChange(PpuState, PpuState),
    ProcessingState(PpuState),
    LcdOff,
    StatInterrupt,
}

#[derive(Clone, Copy)]
enum StatInterrupt {
    ModeInt(PpuState),
    LycInt,
    WriteInt,
    Low,
}

#[deny(unreachable_patterns)]
impl PPU {
    pub fn new() -> Self {
        PPU {
            pixels_processed: 0,
            mode: PpuState::OamSearch,
            tile_block_a: [0; 2048],
            tile_block_b: [0; 2048],
            tile_block_c: [0; 2048],
            tile_map_a: [0; 1024],
            tile_map_b: [0; 1024],
            oam: [0; 160],
            registers: [0; 12],
            invalid: 0xFF,
            ticks: 0,
            stat_line: Low,
            last_render: LcdOff
        }
    }

    pub fn render_cycle(&mut self, cpu_cycles: u8) -> RenderResult {
        if self.display_enabled() {
            *self.line() = 0;
            self.ticks = 0;
            self.mode = OamSearch;
            self.last_render = RenderResult::LcdOff;
            return self.last_render
        }

        let old_mode = self.mode;

        let mut lyc_stat_check = if self.last_render == RenderResult::LcdOff { self.lyc_check() } else { false };

        self.ticks += (cpu_cycles as u16 * 4);

        self.ticks -= match self.mode {
            PpuState::OamSearch => if self.ticks < 80 { 0 } else {
                self.mode = PixelTransfer;
                80
            }

            PpuState::PixelTransfer => if self.ticks < 172 { 0 } else {
                self.mode = HBlank;
                172
            }

            PpuState::HBlank => if self.ticks < 204 { 0 } else {
                *self.line() += 1;
                lyc_stat_check = self.lyc_check();
                self.mode = if *self.line() == 144 { VBlank } else { OamSearch };
                204
            }

            PpuState::VBlank => if self.ticks < 204 + 172 + 80 { 0 } else {
                *self.line() = (*self.line() + 1) % 154;
                lyc_stat_check = self.lyc_check();
                self.mode = if *self.line() == 0 { OamSearch } else { VBlank };
                204 + 172 + 80
            }
        };

        let new_interrupts = self.stat_interrupts(lyc_stat_check);
        let trigger_stat_interrupt = match (self.stat_line, new_interrupts) {
            (Low, [.., Some(ModeInt(m))]) if m == self.mode && old_mode != m => true,
            (Low, [.., Some(LycInt)]) => true,
            _ => false
        };

        self.stat_line = *new_interrupts.iter().find(|i| i.is_some()).map(|i| i.as_ref()).flatten().unwrap_or(&Low);

        self.last_render = match (old_mode, self.mode, trigger_stat_interrupt) {
            (_, VBlank, _) if old_mode != VBlank => RenderResult::StateChange(old_mode, self.mode),
            (_, _, true) => RenderResult::StatInterrupt,
            _ => RenderResult::StateChange(old_mode, self.mode)
        };

        return self.last_render
    }

    pub fn read(&self, address: usize) -> Option<&u8> {
        match (address, self.mode) {
            (0x8000..=0x9FFF, s) if s == PixelTransfer => Some(&0xFF),
            (0x8000..=0x87FF, _) => Some(&self.tile_block_a[(address - 0x8000) as usize]),
            (0x8800..=0x8FFF, _) => Some(&self.tile_block_b[(address - 0x8800) as usize]),
            (0x9000..=0x97FF, _) => Some(&self.tile_block_c[(address - 0x9000) as usize]),
            (0x9800..=0x9BFF, _) => Some(&self.tile_map_a[(address - 0x9800) as usize]),
            (0x9C00..=0x9FFF, _) => Some(&self.tile_map_b[(address - 0x9C00) as usize]),

            (0xFE00..=0xFE9F, s) if s == OamSearch || s == PixelTransfer => Some(&0xFF),
            (0xFE00..=0xFE9F, _) => Some(&self.oam[(address - 0xFE00) as usize]),
            (0xFF40..=0xFF4B, _) => Some(&self.registers[(address - 0xFF40) as usize]),
            _ => None
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        let mut wrote = true;
        match (address, self.mode) {
            (0x8000..=0x87FF, _) => self.tile_block_a[address - 0x8000] = value,
            (0x8800..=0x8FFF, _) => self.tile_block_b[address - 0x8800] = value,
            (0x9000..=0x97FF, _) => self.tile_block_c[address - 0x9000] = value,
            (0x9800..=0x9BFF, _) => self.tile_map_a[address - 0x9800] = value,
            (0x9C00..=0x9FFF, _) => self.tile_map_b[address - 0x9C00] = value,

            (0xFE00..=0xFE9F, m) if m == OamSearch || m == PixelTransfer => {}
            (0xFE00..=0xFE9F, _) => self.oam[address - 0xFE00] = value,

            (0xFF40, _) => *self.lcdc() = value,
            (0xFF41, _) => *self.stat() =
                (value & 0xF8) | match self.mode {
                    HBlank => 0,
                    VBlank => 1,
                    OamSearch => 2,
                    PixelTransfer => 3
                } | if self.lyc_check() { 0x04 } else { 0x0 },

            (0xFF42, _) => *self.scy() = value,

            (0xFF43, _) => *self.scx() = value,

            (0xFF44, _) => {},

            (0xFF45..=0xFF4B, _) => self.registers[address - 0xFF40] = value,

            _ => { wrote = false; }
        }
        wrote
    }

    fn lcdc(&mut self) -> &mut u8 { &mut self.registers[0x0] }

    fn stat(&mut self) -> &mut u8 { &mut self.registers[0x1] }

    fn scy(&mut self) -> &mut u8 { &mut self.registers[0x2] }

    fn scx(&mut self) -> &mut u8 { &mut self.registers[0x3] }

    fn line(&mut self) -> &mut u8 { &mut self.registers[0x4] }

    fn lyc(&mut self) -> &mut u8 { &mut self.registers[0x5] }

    fn bgp(&mut self) -> &mut u8 { &mut self.registers[0x7] }

    fn obp0(&mut self) -> &mut u8 { &mut self.registers[0x8] }

    fn obp1(&mut self) -> &mut u8 { &mut self.registers[0x9] }

    fn wy(&mut self) -> &mut u8 { &mut self.registers[0xA] }

    fn wx(&mut self) -> &mut u8 { &mut self.registers[0xB] }

    fn lyc_check(&mut self) -> bool { *self.line() == *self.lyc() }

    fn display_enabled(&mut self) -> bool { *self.lcdc() & 0x80 == 0 }

    fn stat_interrupts(&mut self, lyc_check: bool) -> [Option<StatInterrupt>; 4] {
        let stat = *self.stat();
        [
            if stat & 0x08 != 0 { Some(ModeInt(OamSearch)) } else { None },
            if stat & 0x10 != 0 { Some(ModeInt(VBlank)) } else { None },
            if stat & 0x20 != 0 { Some(ModeInt(HBlank)) } else { None },
            if stat & 0x40 != 0 && lyc_check { Some(LycInt) } else { None }
        ]
    }
}