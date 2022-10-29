use crate::memory_map::OamCorruptionCause;
use crate::ppu::AddressingMode::{H8000, H8800};
use crate::ppu::DmaState::Inactive;
use crate::ppu::ObjSize::{SingleTile, StackedTile};
use crate::ppu::PpuMode::{HorizontalBlank, OamSearch, PixelTransfer, VerticalBlank};
use crate::ppu::PpuState::{LcdOff, ModeChange, ProcessingMode};
use crate::ppu::RenderCycle::{Normal, StatTrigger};
use crate::ppu::StatInterrupt::{Low, LycInt, ModeInt};
use crate::ppu::TileMapArea::{H9800, H9C00};
use std::cmp::min;
use std::convert::TryInto;
use DmaState::{Executing, Finished, Starting};
use OamCorruptionCause::{IncDec, Read, ReadWrite, Write};

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum DmaState {
    Inactive,
    Starting,
    Executing,
    Finished,
}

#[derive(PartialEq, Copy, Clone, Debug, Ord, PartialOrd, Eq)]
pub enum PpuMode {
    OamSearch,
    PixelTransfer,
    HorizontalBlank,
    VerticalBlank,
}

pub struct PixelProcessingUnit {
    pub mode: PpuMode,
    pub dma: DmaState,
    pub dma_progress: usize,
    pub dma_offset: usize,
    tile_block_a: [u8; 0x8800 - 0x8000],
    tile_block_b: [u8; 0x9000 - 0x8800],
    tile_block_c: [u8; 0x9800 - 0x9000],
    tile_map_a: [u8; 0x9C00 - 0x9800],
    tile_map_b: [u8; 0xA000 - 0x9C00],
    pub oam: [u8; 0xFEA0 - 0xFE00],
    registers: [u8; 0xFF4C - 0xFF41],
    ticks: usize,
    state: PpuState,
    stat_line: StatInterrupt,
    force_irq: bool,
    lcdc: LcdControl,
    pub(crate) pixels: Box<[u32]>,
    pub last_ticks: usize,
    pub old_mode: PpuMode,
    pub last_lyc_check: bool,
    pub oam_corruption: Option<OamCorruptionCause>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum PpuState {
    ModeChange(PpuMode, PpuMode),
    ProcessingMode(PpuMode),
    LcdOff,
}

pub enum RenderCycle {
    Normal(PpuState),
    StatTrigger(PpuState),
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
enum StatInterrupt {
    ModeInt(PpuMode),
    LycInt,
    Low,
}

#[deny(unreachable_patterns)]
impl PixelProcessingUnit {
    pub fn new() -> Self {
        let lcdc = LcdControl::new(0);
        let fb = [0_u32; 160 * 144];
        PixelProcessingUnit {
            mode: HorizontalBlank,
            tile_block_a: [0; 2048],
            tile_block_b: [0; 2048],
            tile_block_c: [0; 2048],
            tile_map_a: [0; 1024],
            tile_map_b: [0; 1024],
            oam: [0; 160],
            registers: [0; 11],
            lcdc,
            oam_corruption: None,
            ticks: 0,
            stat_line: Low,
            state: LcdOff,
            force_irq: true,
            last_ticks: 0,
            dma_progress: 0,
            dma_offset: 0,
            pixels: Box::new(fb),
            old_mode: HorizontalBlank,
            dma: Inactive,
            last_lyc_check: false,
        }
    }

    pub fn machine_cycle(&mut self) -> RenderCycle {
        self.old_mode = self.mode;
        self.ticks += 4;

        if self.dma != Inactive {
            self.dma_cycle();
            self.ticks -= 4;
        }

        if !self.lcdc.enabled() {
            self.reset_state();
            return Normal(self.state);
        } else if self.state == LcdOff {
            self.mode = OamSearch;
            self.old_mode = OamSearch;
        }

        self.handle_mode_transition();
        self.handle_oam_corruption();
        self.handle_lcd_startup();

        //println!("STAT: {} | LYC: {} | LY: {}", self.stat(), self.lyc(), self.ly());
        self.cycle_result()
    }

    fn reset_state(&mut self) {
        self.mode = HorizontalBlank;
        self.old_mode = HorizontalBlank;
        self.last_lyc_check = self.lyc_check();
        *self.ly_mut() = 0;
        self.state = LcdOff;
        self.oam_corruption = None;
        self.force_irq = false;
        self.ticks = 0;
    }

    fn handle_lcd_startup(&mut self) {
        if self.state == LcdOff {
            self.state = ProcessingMode(self.mode);
            self.last_lyc_check = self.lyc_check();
            self.ticks += 2;
        } else {
            self.state = if self.old_mode == self.mode {
                ProcessingMode(self.mode)
            } else {
                ModeChange(self.old_mode, self.mode)
            };
        }
    }

    fn handle_mode_transition(&mut self) {
        self.ticks -= match self.mode {
            OamSearch => {
                if self.ticks < 80 {
                    0
                } else {
                    self.mode = PixelTransfer;
                    80
                }
            }

            PixelTransfer => {
                if self.ticks < 172 {
                    0
                } else {
                    self.mode = HorizontalBlank;
                    172
                }
            }

            HorizontalBlank => {
                if self.ticks < 204 {
                    0
                } else {
                    *self.ly_mut() += 1;
                    self.last_lyc_check = self.lyc_check();
                    self.mode = if self.ly() == 144 {
                        VerticalBlank
                    } else {
                        self.draw_scanline();
                        OamSearch
                    };
                    204
                }
            }

            VerticalBlank => {
                if self.ticks < 456 {
                    0
                } else {
                    *self.ly_mut() += 1;
                    self.last_lyc_check = self.lyc_check();
                    *self.ly_mut() %= 154;
                    self.mode = if *self.ly_mut() == 0 {
                        OamSearch
                    } else {
                        VerticalBlank
                    };
                    456
                }
            }
        };
    }

    fn cycle_result(&mut self) -> RenderCycle {
        let [oam, vblank, hblank, lyc] = self.stat_interrupts();

        let trigger_stat_interrupt = self.stat_line == Low
            && (LycInt == lyc
                || (self.mode != self.old_mode
                    && (ModeInt(OamSearch) == oam
                        || ModeInt(VerticalBlank) == vblank
                        || ModeInt(HorizontalBlank) == hblank)));

        self.stat_line = match [oam, vblank, hblank, lyc] {
            [s @ ModeInt(_), ..] => s,
            [_, s @ ModeInt(_), ..] => s,
            [.., s @ ModeInt(_), _] => s,
            [.., s] => s,
        };

        self.force_irq = false;

        if trigger_stat_interrupt {
            StatTrigger(self.state)
        } else {
            Normal(self.state)
        }
    }

    fn stat_interrupts(&mut self) -> [StatInterrupt; 4] {
        let stat = self.stat();
        if !self.force_irq {
            [
                if stat & 0x08 != 0 {
                    ModeInt(OamSearch)
                } else {
                    Low
                },
                if stat & 0x10 != 0 {
                    ModeInt(VerticalBlank)
                } else {
                    Low
                },
                if stat & 0x20 != 0 {
                    ModeInt(HorizontalBlank)
                } else {
                    Low
                },
                if self.lyc_check() && stat & 0x40 != 0 {
                    LycInt
                } else {
                    Low
                },
            ]
        } else {
            [
                ModeInt(OamSearch),
                ModeInt(VerticalBlank),
                ModeInt(HorizontalBlank),
                if self.lyc_check() { LycInt } else { Low },
            ]
        }
    }

    pub fn read(&mut self, address: usize) -> Option<u8> {
        match (address, self.mode, self.dma) {
            (0x8000..=0x9FFF, PixelTransfer, _) => Some(0xFF),

            (0x8000..=0x87FF, ..) => Some(self.tile_block_a[address - 0x8000]),
            (0x8800..=0x8FFF, ..) => Some(self.tile_block_b[address - 0x8800]),
            (0x9000..=0x97FF, ..) => Some(self.tile_block_c[address - 0x9000]),
            (0x9800..=0x9BFF, ..) => Some(self.tile_map_a[address - 0x9800]),
            (0x9C00..=0x9FFF, ..) => Some(self.tile_map_b[address - 0x9C00]),

            (0xFE00..=0xFE9F, VerticalBlank | HorizontalBlank, Inactive | Starting) => {
                Some(self.oam[address - 0xFE00])
            }

            (0xFE00..=0xFE9F, ..) => {
                self.oam_corruption = match self.oam_corruption {
                    None => Some(Read),
                    Some(IncDec) => Some(ReadWrite),
                    _ => panic!(),
                };
                Some(0xFF)
            }

            (0xFE00..=0xFEFF, ..) => Some(0xFF),

            (0xFF40, ..) => Some(self.lcdc.get()),
            (0xFF41, ..) => Some(self.stat()),
            (0xFF42..=0xFF4B, ..) => Some(self.registers[address - 0xFF41]),
            _ => None,
        }
    }

    pub fn write(&mut self, address: usize, value: u8) -> bool {
        match (address, self.mode, self.dma) {
            (0x8000..=0x9FFF, PixelTransfer, _) => (),
            (0x8000..=0x87FF, ..) => self.tile_block_a[address - 0x8000] = value,
            (0x8800..=0x8FFF, ..) => self.tile_block_b[address - 0x8800] = value,
            (0x9000..=0x97FF, ..) => self.tile_block_c[address - 0x9000] = value,
            (0x9800..=0x9BFF, ..) => self.tile_map_a[address - 0x9800] = value,
            (0x9C00..=0x9FFF, ..) => self.tile_map_b[address - 0x9C00] = value,

            (0xFE00..=0xFEFF, OamSearch, ..) => {
                self.oam_corruption = Some(Write);
            }

            (0xFE00..=0xFE9F, HorizontalBlank | VerticalBlank, Inactive | Starting) => {
                self.oam[address - 0xFE00] = value
            }

            (0xFE00..=0xFEFF, ..) => (),

            (0xFF40, ..) => self.lcdc.set(value),
            (0xFF41, ..) => {
                *self.stat_mut() = value;
                self.force_irq = true
            }

            (0xFF44, ..) => (),

            (0xFF46, ..) => {
                self.dma_offset = value as usize;
                self.dma_cycle();
                self.registers[address - 0xFF41] = value;
            }

            (0xFF42..=0xFF43 | 0xFF45 | 0xFF47..=0xFF4B, ..) => {
                self.registers[address - 0xFF41] = value
            }

            _ => return false,
        }
        true
    }

    fn dma_cycle(&mut self) {
        self.dma = match self.dma {
            Inactive => {
                self.dma_progress = 0;
                Starting
            }
            Executing => {
                self.dma_progress += 1;
                if self.dma_progress == self.oam.len() {
                    Finished
                } else {
                    Executing
                }
            }
            Starting => Executing,
            Finished => Inactive,
        };
    }

    fn stat(&self) -> u8 {
        //println!("LY: {} | LYC: {}, State: {:?} | STAT: {}", self.ly(), self.lyc(), self.state, stat);
        self.registers[0] & 0xF8
            | match (self.mode, self.ticks) {
                (OamSearch, 0..=6) => 0,
                (VerticalBlank, 0..=4) if self.ly() == 144 => 0,
                (HorizontalBlank, _) => 0,
                (VerticalBlank, _) => 1,
                (OamSearch, _) => 2,
                (PixelTransfer, _) => 3,
            }
            | if self.lyc_check() { 0x04 } else { 0x0 }
            | 0x80
    }

    fn stat_mut(&mut self) -> &mut u8 {
        &mut self.registers[0]
    }

    fn scy(&self) -> &u8 {
        &self.registers[1]
    }

    fn scx(&self) -> &u8 {
        &self.registers[2]
    }

    pub fn ly(&self) -> u8 {
        let ly = self.registers[3];
        if ly != 153 || self.ticks <= 4 {
            ly
        } else {
            0
        }
    }

    pub fn ly_mut(&mut self) -> &mut u8 {
        &mut self.registers[3]
    }

    fn lyc(&self) -> &u8 {
        &self.registers[4]
    }

    fn lyc_check(&self) -> bool {
        if self.state == LcdOff {
            return self.last_lyc_check;
        }
        self.ticks > 4
            && (match (self.mode, self.ticks) {
                (VerticalBlank, 5..=8) => 153,
                (VerticalBlank, 9..=12) => !self.lyc(),
                (..) => self.ly(),
            }) == *self.lyc()
    }

    fn bgp(&self) -> &u8 {
        &self.registers[6]
    }

    fn obp0(&self) -> &u8 {
        &self.registers[7]
    }

    fn obp1(&self) -> &u8 {
        &self.registers[8]
    }

    fn wy(&self) -> &u8 {
        &self.registers[9]
    }

    fn wx(&self) -> &u8 {
        &self.registers[10]
    }

    fn render_background_window(&mut self) {
        let scx = *self.scx();
        let scy = *self.scy();
        let wx = self.wx().wrapping_sub(7);
        let wy = *self.wy();
        let ly = self.ly();

        let use_window = wy <= ly && self.lcdc.window_enabled();

        let vertical_position = if use_window {
            ly.wrapping_sub(wy)
        } else {
            scy.wrapping_add(ly)
        } as usize;

        let tile_row = (vertical_position / 8) as usize * 32;

        for pixel in 0..160 {
            let horizontal_position = if use_window && pixel >= wx {
                pixel.wrapping_sub(wx)
            } else {
                pixel.wrapping_add(scx)
            };

            let background_area = if use_window && pixel >= wx {
                self.lcdc.window_tile_map_area()
            } else {
                self.lcdc.background_tile_map_area()
            } as usize;

            let tile_col = (horizontal_position / 8) as usize;

            let tile_address = background_area + tile_row + tile_col;

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

            let color_bit = -((horizontal_position as i32 % 8) - 7);

            let color_num = ((data2 >> color_bit) & 0b1) << 1;
            let color_num = color_num | ((data1 >> color_bit) & 0b1);

            let color = self.get_color(color_num, *self.bgp());
            self.set_pixel(pixel as u32, ly as u32, color)
        }
    }

    fn draw_scanline(&mut self) {
        if self.lcdc.background_window_enabled() {
            self.render_background_window()
        }
        if self.lcdc.sprite_enabled() {
            self.render_sprites()
        }
    }

    fn render_sprites(&mut self) {
        let ly = self.ly();
        let tile_length = self.lcdc.object_size() as u8;

        if ly > 143 {
            return;
        }

        for sprite_index in (0..160).step_by(4) {
            let sprite = Sprite::new(self, sprite_index);

            if ly >= sprite.vertical_position
                && ly < (sprite.vertical_position.wrapping_add(tile_length))
            {
                let line: i32 = ly as i32 - sprite.vertical_position as i32;
                let line = (if sprite.attributes.flipped_vertically {
                    -(line - tile_length as i32)
                } else {
                    line
                }) as u16
                    * 2;

                let data_address = 0x8000 + ((sprite.location * 16) + line) as usize;

                let pixel_data_left = self.read(data_address).unwrap();
                let pixel_data_right = self.read(data_address + 1).unwrap();

                for tile_pixel in (0..8).rev() {
                    let color_bit = tile_pixel as i32;
                    let color_bit = if sprite.attributes.flipped_horizontally {
                        -(color_bit - 7)
                    } else {
                        color_bit
                    };

                    let color_num = (((pixel_data_right >> color_bit) & 0b1) << 1)
                        | ((pixel_data_left >> color_bit) & 0b1);

                    if color_num == 0 {
                        continue;
                    }

                    let color = self.get_color(color_num, sprite.attributes.palette);

                    let x_pix = 0_u8.wrapping_sub(tile_pixel as u8).wrapping_add(7);

                    let pixel = sprite.horizontal_position.wrapping_add(x_pix);

                    if pixel > 159 {
                        continue;
                    }

                    self.set_sprite_pixel(
                        pixel as u32,
                        ly as u32,
                        sprite.attributes.obj_to_background_priority,
                        color,
                    )
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
        let [a, r, g, b] = self.pixels[offset].to_be_bytes();
        let pixel = Color { a, r, g, b };

        if pixel != WHITE && pri {
        } else {
            self.set_pixel(x, y, color)
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        let offset = (y * 160 + x) as usize;

        self.pixels[offset] = u32::from_be_bytes([color.a, color.r, color.g, color.b]);
    }

    fn handle_oam_corruption(&mut self) {
        if self.mode != OamSearch {
            self.oam_corruption = None;
            return;
        }

        // println!("{:?}", self.oam_corruption);
        match self.oam_corruption {
            Some(Write | IncDec) => self.handle_oam_write_corruption(),
            Some(Read) => self.handle_oam_read_corruption(),
            Some(ReadWrite) => self.handle_oam_read_write_corruption(),
            _ => (),
        };
        self.oam_corruption = None;
    }

    fn handle_oam_read_write_corruption(&mut self) {
        // TODO: ReadWrite behavior seems different than Read/Write/IncDec
    }

    fn handle_oam_read_corruption(&mut self) {
        self.handle_oam_pattern_corruption(|a, b, c| b | (a & c));
    }

    fn handle_oam_write_corruption(&mut self) {
        self.handle_oam_pattern_corruption(|a, b, c| ((a ^ c) & (b ^ c)) ^ c);
    }

    fn handle_oam_pattern_corruption(&mut self, pattern: fn(u16, u16, u16) -> u16) {
        let oam_row = min(19, self.ticks / 4);
        if oam_row == 0 {
            return;
        }
        let mut rows = self.oam.chunks_mut(8);

        let (previous_row, current_row) = (rows.nth(oam_row - 1).unwrap(), rows.next().unwrap());

        let a = u16::from_le_bytes(current_row[0..2].as_ref().try_into().unwrap());
        let b = u16::from_le_bytes(previous_row[0..2].as_ref().try_into().unwrap());
        let c = u16::from_le_bytes(previous_row[4..6].as_ref().try_into().unwrap());

        let pattern = pattern(a, b, c).to_le_bytes();
        current_row[0..2].clone_from_slice(pattern.as_slice());
        current_row[2..].clone_from_slice(&previous_row[2..]);
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
    fn new(register: u8) -> Self {
        Self { reg: register }
    }
    fn enabled(&self) -> bool {
        self.reg & 0x80 != 0
    }
    fn window_tile_map_area(&self) -> TileMapArea {
        if self.reg & 0x40 != 0 {
            H9C00
        } else {
            H9800
        }
    }
    fn window_enabled(&self) -> bool {
        self.reg & 0x20 != 0 && self.reg & 0x01 != 0
    }
    fn addressing_mode(&self) -> AddressingMode {
        if self.reg & 0x10 != 0 {
            H8000
        } else {
            H8800
        }
    }
    fn background_tile_map_area(&self) -> TileMapArea {
        if self.reg & 0x08 != 0 {
            H9C00
        } else {
            H9800
        }
    }
    fn object_size(&self) -> ObjSize {
        if self.reg & 0x04 != 0 {
            StackedTile
        } else {
            SingleTile
        }
    }
    fn sprite_enabled(&self) -> bool {
        self.reg & 0x02 != 0
    }
    fn background_window_enabled(&self) -> bool {
        self.reg & 0x01 != 0
    }
    fn get(&self) -> u8 {
        self.reg
    }
    fn set(&mut self, value: u8) {
        self.reg = value
    }
}

struct Sprite {
    vertical_position: u8,
    horizontal_position: u8,
    location: u16,
    attributes: SpriteAttributes,
}

impl Sprite {
    fn new(ppu: &PixelProcessingUnit, index: usize) -> Self {
        Self {
            vertical_position: ppu.oam[index].wrapping_sub(16),
            horizontal_position: ppu.oam[index + 1].wrapping_sub(8),
            location: ppu.oam[index + 2] as u16,
            attributes: SpriteAttributes::new(ppu, ppu.oam[index + 3]),
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
    fn new(ppu: &PixelProcessingUnit, attrs: u8) -> Self {
        Self {
            flipped_vertically: attrs & 0x40 != 0,
            flipped_horizontally: attrs & 0x20 != 0,
            obj_to_background_priority: attrs & 0x80 != 0,
            palette: *if attrs & 0x10 != 0 {
                ppu.obp1()
            } else {
                ppu.obp0()
            },
        }
    }
}
