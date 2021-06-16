use std::cmp::max;
use std::ops::{Index, IndexMut, Range, RangeInclusive};

use crate::memory_map::{MemoryMap};
use crate::ppu::PpuMode::{HBlank, OamSearch, PixelTransfer, VBlank};
use crate::interrupt::{Interrupt, InterruptId};
use crate::ppu::StatInterrupt::{Low, ModeInt, LycInt, WriteInt};
use crate::ppu::PpuState::{LcdOff, ProcessingMode, ModeChange};
use crate::ppu::TileMapArea::{H9C00, H9800};
use crate::ppu::ObjSize::{StackedTile, SingleTile};
use crate::ppu::AddressingMode::{H8800, H8000};
use crate::ppu::RenderCycle::{Normal, StatTrigger};

enum PpuRegisterId { LcdControl, LcdStatus, LcdInterrupt, ScrollY, ScrollX, ScanLine, Background }

enum PpuRegisterAccess { R, W, RW }

type PpuRegisterAddress = u16;

struct PpuRegister(PpuRegisterAddress, u8, PpuRegisterId, PpuRegisterAccess);

#[derive(PartialEq, Copy, Clone)]
pub enum PpuMode {
    OamSearch,
    PixelTransfer,
    HBlank,
    VBlank,
}

pub struct PPU {
    pixels_processed: u16,
    mode: PpuMode,
    tile_block_a: [u8; 0x8800 - 0x8000],
    tile_block_b: [u8; 0x9000 - 0x8800],
    tile_block_c: [u8; 0x9800 - 0x9000],
    tile_map_a: [u8; 0x9C00 - 0x9800],
    tile_map_b: [u8; 0xA000 - 0x9C00],
    oam: [u8; 0xFEA0 - 0xFE00],
    registers: [u8; 0xFF4C - 0xFF41],
    ticks: u16,
    state: PpuState,
    stat_line: StatInterrupt,
    force_irq: bool,
    lcdc: LcdControl
}

#[derive(PartialEq, Clone, Copy)]
pub enum PpuState {
    ModeChange(PpuMode, PpuMode),
    ProcessingMode(PpuMode),
    LcdOff,
}

pub enum RenderCycle {
    Normal(PpuState),
    StatTrigger(PpuState)
}

#[derive(Clone, Copy)]
enum StatInterrupt {
    ModeInt(PpuMode),
    LycInt,
    WriteInt,
    Low,
}

#[deny(unreachable_patterns)]
impl PPU {
    pub fn new() -> Self {
        PPU {
            pixels_processed: 0,
            mode: PpuMode::OamSearch,
            tile_block_a: [0; 2048],
            tile_block_b: [0; 2048],
            tile_block_c: [0; 2048],
            tile_map_a: [0; 1024],
            tile_map_b: [0; 1024],
            oam: [0; 160],
            registers: [0; 11],
            lcdc: LcdControl::new(0),
            ticks: 0,
            stat_line: Low,
            state: LcdOff,
            force_irq: true,
        }
    }

    pub fn render_cycle(&mut self, cpu_cycles: u8) -> RenderCycle {
        if !self.lcdc.enabled() {
            *self.ly() = 0;
            self.ticks = 0;
            self.mode = HBlank;
            self.state = PpuState::LcdOff;
            self.force_irq = false;
            return Normal(self.state);
        }

        let old_mode = self.mode;

        if self.state == PpuState::LcdOff {
            self.mode = OamSearch;
        }

        let mut lyc_stat_check = if self.state == PpuState::LcdOff { self.lyc_check() } else { false };

        self.ticks += (cpu_cycles as u16 * 4);

        self.ticks -= match self.mode {
            PpuMode::OamSearch => if self.ticks < 80 { 0 } else {
                self.mode = PixelTransfer;
                80
            }

            PpuMode::PixelTransfer => if self.ticks < 172 { 0 } else {
                self.mode = HBlank;
                172
            }

            PpuMode::HBlank => if self.ticks < 204 { 0 } else {
                *self.ly() += 1;
                lyc_stat_check = self.lyc_check();
                self.mode = if *self.ly() == 144 { VBlank } else { OamSearch };
                204
            }

            PpuMode::VBlank => if self.ticks < 204 + 172 + 80 { 0 } else {
                *self.ly() = (*self.ly() + 1) % 154;
                lyc_stat_check = self.lyc_check();
                self.mode = if *self.ly() == 0 { OamSearch } else { VBlank };
                204 + 172 + 80
            }
        };
        self.state = if old_mode == self.mode { ProcessingMode(self.mode) } else { ModeChange(old_mode, self.mode)};

        self.cycle_result(old_mode, lyc_stat_check)
    }

    fn cycle_result(&mut self, old_mode: PpuMode, lyc_stat_check: bool) -> RenderCycle {
        let new_interrupts = self.stat_interrupts(lyc_stat_check);
        let trigger_stat_interrupt = match (self.stat_line, new_interrupts) {
            (Low, [.., Some(ModeInt(m))]) if m == self.mode && old_mode != m => true,
            (Low, [.., Some(LycInt)]) => true,
            _ => false
        };
        self.stat_line = *new_interrupts.iter().find(|i| i.is_some()).map(|i| i.as_ref()).flatten().unwrap_or(&Low);
        self.force_irq = false;
        if trigger_stat_interrupt { StatTrigger(self.state) } else { Normal(self.state) }
    }

    fn stat_interrupts(&mut self, lyc_check: bool) -> [Option<StatInterrupt>; 4] {
        let stat = *self.stat();
        [
            if stat & 0x08 != 0 || self.force_irq { Some(ModeInt(OamSearch)) } else { None },
            if stat & 0x10 != 0 || self.force_irq { Some(ModeInt(VBlank)) } else { None },
            if stat & 0x20 != 0 || self.force_irq { Some(ModeInt(HBlank)) } else { None },
            if lyc_check && (stat & 0x40 != 0 || self.force_irq) { Some(LycInt) } else { None }
        ]
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
            (0xFF40, _) => Some(self.lcdc.get()),
            (0xFF41..=0xFF4B, _) => Some(&self.registers[(address - 0xFF41) as usize]),
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

            (0xFF40, _) => self.lcdc.set(value),
            (0xFF41, _) => {
                *self.stat() = (value & 0xF8) | match self.mode {
                    HBlank => 0,
                    VBlank => 1,
                    OamSearch => 2,
                    PixelTransfer => 3
                } | if self.lyc_check() { 0x04 } else { 0x0 };
                self.force_irq = true
            }

            (0xFF44, _) => {}

            (0xFF42..=0xFF43, _) | (0xFF45..=0xFF4B, _) => self.registers[address - 0xFF41] = value,

            _ => { wrote = false; }
        }
        wrote
    }

    fn stat(&mut self) -> &mut u8 { &mut self.registers[0x0] }

    fn scy(&mut self) -> &mut u8 { &mut self.registers[0x1] }

    fn scx(&mut self) -> &mut u8 { &mut self.registers[0x2] }

    pub(crate) fn ly(&mut self) -> &mut u8 { &mut self.registers[0x3] }

    fn lyc(&mut self) -> &mut u8 { &mut self.registers[0x4] }

    fn bgp(&mut self) -> &mut u8 { &mut self.registers[0x6] }

    fn obp0(&mut self) -> &mut u8 { &mut self.registers[0x7] }

    fn obp1(&mut self) -> &mut u8 { &mut self.registers[0x8] }

    fn wy(&mut self) -> &mut u8 { &mut self.registers[0x9] }

    fn wx(&mut self) -> &mut u8 { &mut self.registers[0xA] }

    fn lyc_check(&mut self) -> bool { *self.ly() == *self.lyc() }
}

struct LcdControl {
    reg: u8,
}

enum TileMapArea {
    H9800,
    H9C00,
}

enum AddressingMode {
    H8000,
    H8800
}

enum ObjSize {
    SingleTile, StackedTile
}

impl LcdControl {
    fn new(register: u8) -> Self { Self { reg: register } }

    fn enabled(&self) -> bool { self.reg & 0x80 != 0 }
    fn w_tile_map_area(&self) -> TileMapArea { if self.reg & 0x40 != 0 { H9C00 } else { H9800 } }
    fn window_enable(&self) -> bool { self.reg & 0x20 != 0 && self.reg & 0x01 != 0 }
    fn addressing_mode(&self) -> AddressingMode { if self.reg & 0x10 != 0 { H8800 } else {H8000} }
    fn bg_tile_map_area(&self) -> TileMapArea { if self.reg & 0x08 != 0 { H9C00 } else { H9800 } }
    fn obj_size(&self) -> ObjSize { if self.reg & 0x04 != 0 { StackedTile } else { SingleTile } }
    fn obj_enable(&self) -> bool { self.reg & 0x02 != 0 }
    fn bg_window_enable(&self) -> bool { self.reg & 0x01 != 0 }
    fn get(&self) -> &u8 {
        &self.reg
    }
    fn set(&mut self, value: u8) {
        self.reg = value
    }
}