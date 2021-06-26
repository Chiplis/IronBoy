use std::cmp::max;
use std::ops::{Index, IndexMut, Range, RangeInclusive};

use crate::memory_map::{MemoryMap};
use crate::ppu::PpuMode::{HBlank, OamSearch, PixelTransfer, VBlank};
use crate::interrupt::{InterruptHandler, InterruptId};
use crate::ppu::StatInterrupt::{Low, ModeInt, LycInt, WriteInt};
use crate::ppu::PpuState::{LcdOff, ProcessingMode, ModeChange};
use crate::ppu::TileMapArea::{H9C00, H9800};
use crate::ppu::ObjSize::{StackedTile, SingleTile};
use crate::ppu::AddressingMode::{H8800, H8000};
use crate::ppu::RenderCycle::{Normal, StatTrigger};
use std::collections::VecDeque;
use minifb::{Key, WindowOptions, Window, ScaleMode, Scale, InputCallback};

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
    ticks: usize,
    state: PpuState,
    stat_line: StatInterrupt,
    force_irq: bool,
    lcdc: LcdControl,
    pixels: Box<[u32]>,
    pub window: Window,
    pub(crate) last_ticks: usize,
}

#[derive(PartialEq, Clone, Copy)]
pub enum PpuState {
    ModeChange(PpuMode, PpuMode),
    ProcessingMode(PpuMode),
    LcdOff,
}

pub enum RenderCycle {
    Normal(PpuState),
    StatTrigger(PpuState),
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
        let lcdc = LcdControl::new(0);
        let fb = [0_u32; 166 * 144];
        let mut window = Window::new(
            "Test - ESC to exit",
            160,
            144,
            WindowOptions {
                borderless: false,
                transparency: false,
                title: true,
                resize: true,
                scale: Scale::X1,
                scale_mode: ScaleMode::Stretch,
                topmost: false,
                none: false,
            }).unwrap();
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
            lcdc,
            ticks: 0,
            stat_line: Low,
            state: LcdOff,
            force_irq: true,
            last_ticks: 0,
            pixels: Box::new(fb),
            window,
        }
    }

    pub fn render_cycle(&mut self, cpu_cycles: usize) -> RenderCycle {
        if !self.lcdc.enabled() {
            *self.ly_mut() = 0;
            self.ticks = 0;
            self.mode = HBlank;
            self.state = PpuState::LcdOff;
            self.force_irq = false;
            return Normal(self.state);
        }

        let old_mode = self.mode;
        let mut lyc_stat_check = false;

        if self.state == PpuState::LcdOff {
            self.mode = OamSearch;
            lyc_stat_check = self.lyc_check();
        }

        self.last_ticks = cpu_cycles as usize * 4;
        self.ticks += (cpu_cycles as usize * 4);

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
                *self.ly_mut() += 1;
                lyc_stat_check = self.lyc_check();
                self.mode = if *self.ly() == 144 {
                    self.window.update_with_buffer(&self.pixels, 160, 144).unwrap();
                    VBlank
                } else {
                    self.draw_scanline();
                    OamSearch
                };
                204
            }

            PpuMode::VBlank => if self.ticks < 456 { 0 } else {
                *self.ly_mut() = (*self.ly() + 1) % 154;
                lyc_stat_check = self.lyc_check();
                self.mode = if *self.ly() == 0 { OamSearch } else { VBlank };
                456
            }
        };
        self.state = if old_mode == self.mode { ProcessingMode(self.mode) } else { ModeChange(old_mode, self.mode) };

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
        let stat = *self.stat_mut();
        [
            if stat & 0x08 != 0 || self.force_irq { Some(ModeInt(OamSearch)) } else { None },
            if stat & 0x10 != 0 || self.force_irq { Some(ModeInt(VBlank)) } else { None },
            if stat & 0x20 != 0 || self.force_irq { Some(ModeInt(HBlank)) } else { None },
            if lyc_check && (stat & 0x40 != 0 || self.force_irq) { Some(LycInt) } else { None }
        ]
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        match (address, self.mode) {
            (0x8000..=0x9FFF, PixelTransfer) => Some(0xFF),
            (0x8000..=0x87FF, _) => Some(self.tile_block_a[(address - 0x8000) as usize]),
            (0x8800..=0x8FFF, _) => Some(self.tile_block_b[(address - 0x8800) as usize]),
            (0x9000..=0x97FF, _) => Some(self.tile_block_c[(address - 0x9000) as usize]),
            (0x9800..=0x9BFF, _) => Some(self.tile_map_a[(address - 0x9800) as usize]),
            (0x9C00..=0x9FFF, _) => Some(self.tile_map_b[(address - 0x9C00) as usize]),

            (0xFE00..=0xFE9F, PixelTransfer | OamSearch) => Some(0xFF),
            (0xFE00..=0xFE9F, _) => Some(self.oam[(address - 0xFE00) as usize]),
            (0xFF40, _) => Some(self.lcdc.get()),
            (0xFF41, _) => Some(self.stat()),
            (0xFF42..=0xFF4B, _) => Some(self.registers[(address - 0xFF41) as usize]),
            _ => None
        }
    }

    pub fn write(&mut self, ram: [u8; 0x10000], address: usize, value: u8) -> bool {
        match (address, self.mode) {
            (0x8000..=0x87FF, _) => self.tile_block_a[address - 0x8000] = value,
            (0x8800..=0x8FFF, _) => self.tile_block_b[address - 0x8800] = value,
            (0x9000..=0x97FF, _) => self.tile_block_c[address - 0x9000] = value,
            (0x9800..=0x9BFF, _) => self.tile_map_a[address - 0x9800] = value,
            (0x9C00..=0x9FFF, _) => self.tile_map_b[address - 0x9C00] = value,

            (0xFE00..=0xFE9F, OamSearch | PixelTransfer) => {}
            (0xFE00..=0xFE9F, _) => self.oam[address - 0xFE00] = value,

            (0xFF40, _) => self.lcdc.set(value),
            (0xFF41, _) => {
                *self.stat_mut() = value;
                self.force_irq = true
            }

            (0xFF44, _) => {}

            (0xFF46, _) => {
                for i in 0..self.oam.len() {
                    self.oam[i] = ram[(value as usize * 0x100 + i as usize)]
                }
                self.registers[address - 0xFF41] = value
            }

            (0xFF42..=0xFF43 | 0xFF45 | 0xFF47..=0xFF4B, _) => self.registers[address - 0xFF41] = value,

            _ => return false
        }
        true
    }

    fn stat(&self) -> u8 {
        self.registers[0] & 0xF8 | match self.mode {
            HBlank => 0,
            VBlank => 1,
            OamSearch => 2,
            PixelTransfer => 3
        } | if self.lyc_check() { 0x04 } else { 0x0 }
    }

    fn stat_mut(&mut self) -> &mut u8 { &mut self.registers[0] }

    fn scy(&self) -> &u8 { &self.registers[1] }

    fn scx(&self) -> &u8 { &self.registers[2] }

    pub fn ly(&self) -> &u8 { &self.registers[3] }

    pub fn ly_mut(&mut self) -> &mut u8 { &mut self.registers[3] }

    fn lyc(&self) -> &u8 { &self.registers[4] }

    fn bgp(&self) -> &u8 { &self.registers[6] }

    fn obp0(&self) -> &u8 { &self.registers[7] }

    fn obp1(&self) -> &u8 { &self.registers[8] }

    fn wy(&self) -> &u8 { &self.registers[9] }

    fn wx(&self) -> &u8 { &self.registers[10] }

    fn lyc_check(&self) -> bool { *self.ly() == *self.lyc() }

    fn render_background_window(&mut self) {
        let scx = *self.scx();
        let scy = *self.scy();
        let wx = self.wx().wrapping_sub(7);
        let wy = *self.wy();
        let ly = *self.ly();


        let use_window = wy <= ly && self.lcdc.window_enabled();

        let background_area = if use_window { self.lcdc.window_tile_map_area() } else { self.lcdc.background_tile_map_area() } as usize;

        let vertical_position = if use_window { ly.wrapping_sub(wy) } else { scy.wrapping_add(ly) } as usize;

        let tile_row = (vertical_position / 8) as usize * 32;

        for pixel in 0..160 {
            let horizontal_position = if use_window && pixel >= wx { pixel.wrapping_sub(wx) } else { pixel.wrapping_add(scx) };

            let tile_col = (horizontal_position / 8) as usize;

            let tile_address = (background_area + tile_row + tile_col);

            let tile_offset: i16 = if self.lcdc.addressing_mode() == H8000 {
                self.read(tile_address).unwrap() as u16 as i16
            } else {
                self.read(tile_address).unwrap() as i8 as i16
            };

            let tile_location = if self.lcdc.addressing_mode() == H8000 {
                self.lcdc.addressing_mode() as usize + (tile_offset as usize * 16)
            } else {
                self.lcdc.addressing_mode() as usize + ((tile_offset + 128) * 16) as usize
            };

            let line: usize = (vertical_position % 8) * 2;
            let data1 = self.read((tile_location + line) as usize).unwrap();
            let data2 = self.read((tile_location + line + 1) as usize).unwrap();

            let color_bit = ((horizontal_position as i32 % 8) - 7) * -1;

            let color_num = ((data2 >> color_bit) & 0b1) << 1;
            let color_num = color_num | ((data1 >> color_bit) & 0b1);

            let color = self.get_color(color_num, *self.bgp());
            self.set_pixel(pixel as u32, ly as u32, color)
        }
    }

    fn draw_scanline(&mut self) {
        if self.lcdc.background_window_enabled() { self.render_background_window() }
        if self.lcdc.sprite_enabled() { self.render_sprites() }
    }

    fn render_sprites(&mut self) {
        let ly = *self.ly();
        let tile_length = self.lcdc.object_size() as u8;

        if ly > 143 { return; }

        for sprite_index in (0..160).step_by(4) {
            let sprite = Sprite::new(self, sprite_index);

            if ly >= sprite.vertical_position && ly < (sprite.vertical_position.wrapping_add(tile_length)) {
                let line: i32 = ly as i32 - sprite.vertical_position as i32;
                let line = (if sprite.attributes.flipped_vertically {
                    (line - tile_length as i32) * -1
                } else {
                    line
                }) as u16 * 2;

                let data_address = 0x8000 + ((sprite.location * 16) + line) as usize;

                let pixel_data_left = self.read(data_address).unwrap();
                let pixel_data_right = self.read(data_address + 1).unwrap();

                for tile_pixel in (0..8).rev() {
                    let color_bit = tile_pixel as i32;
                    let color_bit = if sprite.attributes.flipped_horizontally {
                        (color_bit - 7) * -1
                    } else {
                        color_bit
                    };

                    let color_num = (((pixel_data_right >> color_bit) & 0b1) << 1) | ((pixel_data_left >> color_bit) & 0b1);

                    if color_num == 0 { continue; }

                    let color = self.get_color(color_num, sprite.attributes.palette);

                    let x_pix = 0_u8.wrapping_sub(tile_pixel as u8).wrapping_add(7);

                    let pixel = sprite.horizontal_position.wrapping_add(x_pix);

                    if pixel > 159 { continue; }

                    self.set_sprite_pixel(pixel as u32, ly as u32, sprite.attributes.obj_to_background_priority, color)
                }
            }
        }
    }

    fn get_color(&self, color_id: u8, palette_num: u8) -> Color {
        let (hi, lo) = match color_id {
            0 => (1, 0),
            1 => (3, 2),
            2 => (5, 4),
            3 => (7, 6),
            _ => panic!("Invalid color id: 0x{:x}", color_id),
        };

        let color = ((palette_num >> hi) & 0b1) << 1;
        let color = color | ((palette_num >> lo) & 0b1);

        match color {
            0 => WHITE,
            1 => LIGHT_GRAY,
            2 => DARK_GRAY,
            3 => BLACK,
            _ => panic!("Invalid color: 0x{:x}", color),
        }
    }

    fn set_sprite_pixel(&mut self, x: u32, y: u32, pri: bool, color: Color) {
        let offset = ((y * 160) + x) as usize;
        let pixel = Color {
            a: (self.pixels[offset] >> 0x18) as u8,
            r: (self.pixels[offset] >> 0x10) as u8,
            g: (self.pixels[offset] >> 0x08) as u8,
            b: self.pixels[offset] as u8,
        };

        if pixel != WHITE && pri {
            return;
        } else {
            self.set_pixel(x, y, color)
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        let offset = (y * 160 + x) as usize;

        self.pixels[offset] = ((color.a as u32) << 24) | ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

const WHITE: Color = Color {
    r: 224,
    g: 248,
    b: 208,
    a: 255,
};
const LIGHT_GRAY: Color = Color {
    r: 136,
    g: 192,
    b: 112,
    a: 255,
};
const DARK_GRAY: Color = Color {
    r: 39,
    g: 80,
    b: 70,
    a: 255,
};
const BLACK: Color = Color {
    r: 8,
    g: 24,
    b: 32,
    a: 255,
};

#[derive(PartialEq, Ord, PartialOrd, Eq)]
struct LcdControl {
    reg: u8,
}

#[derive(PartialEq, Ord, PartialOrd, Eq)]
enum TileMapArea {
    H9800 = 0x9800,
    H9C00 = 0x9C00,
}

#[derive(PartialEq, Ord, PartialOrd, Eq)]
enum AddressingMode {
    H8000 = 0x8000,
    H8800 = 0x8800,
}

#[derive(PartialEq, Ord, PartialOrd, Eq)]
enum ObjSize {
    SingleTile = 8,
    StackedTile = 16,
}

impl LcdControl {
    fn new(register: u8) -> Self { Self { reg: register } }
    fn enabled(&self) -> bool { self.reg & 0x80 != 0 }
    fn window_tile_map_area(&self) -> TileMapArea { if self.reg & 0x40 != 0 { H9C00 } else { H9800 } }
    fn window_enabled(&self) -> bool { self.reg & 0x20 != 0 && self.reg & 0x01 != 0 }
    fn addressing_mode(&self) -> AddressingMode { if self.reg & 0x10 != 0 { H8000 } else { H8800 } }
    fn background_tile_map_area(&self) -> TileMapArea { if self.reg & 0x08 != 0 { H9C00 } else { H9800 } }
    fn object_size(&self) -> ObjSize { if self.reg & 0x04 != 0 { StackedTile } else { SingleTile } }
    fn sprite_enabled(&self) -> bool { self.reg & 0x02 != 0 }
    fn background_window_enabled(&self) -> bool { self.reg & 0x01 != 0 }
    fn get(&self) -> u8 { self.reg }
    fn set(&mut self, value: u8) { self.reg = value }
}


struct Sprite {
    vertical_position: u8,
    horizontal_position: u8,
    location: u16,
    attributes: SpriteAttributes,
}

impl Sprite {
    fn new(ppu: &PPU, index: usize) -> Self {
        Self {
            vertical_position: ppu.oam[index].wrapping_sub(16),
            horizontal_position: ppu.oam[index + 1].wrapping_sub(8),
            location: ppu.oam[index + 2] as u16,
            attributes: SpriteAttributes::new(&ppu, ppu.oam[index + 3]),
        }
    }
}

struct SpriteAttributes {
    flipped_vertically: bool,
    flipped_horizontally: bool,
    obj_to_background_priority: bool,
    palette: u8,
}

impl SpriteAttributes {
    fn new(ppu: &PPU, attrs: u8) -> Self {
        Self {
            flipped_vertically: attrs & 0x40 != 0,
            flipped_horizontally: attrs & 0x20 != 0,
            obj_to_background_priority: attrs & 0x80 != 0,
            palette: *if attrs & 0x10 != 0 { ppu.obp1() } else { ppu.obp0() },
        }
    }
}