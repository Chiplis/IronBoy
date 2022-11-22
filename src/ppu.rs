use crate::mmu::{MemoryArea, OamCorruptionCause};
use OamCorruptionCause::{IncDec, Read, ReadWrite, Write};

use serde::{Deserialize, Serialize};

use HorizontalBlankPhase::*;
use OamSearchPhase::*;
use PixelTransferPhase::*;
use PpuState::*;
use VerticalBlankPhase::*;

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PixelProcessingUnit {
    oam_start_clock_count: usize,
    pub(crate) oam_corruption: Option<OamCorruptionCause>,
    /// 8000-9FFF: Video RAM
    pub vram: Vec<u8>,
    /// FE00-FE9F: Sprite Attribute table
    pub oam: Vec<u8>,
    pub dma: u8,
    /// The cycle in which the last DMA transfer was requested.
    pub(crate) dma_started: usize,
    /// If the DMA is running, including the initial delay.
    pub(crate) dma_running: bool,
    /// Oam read is blocked
    pub(crate) dma_block_oam: bool,

    pub(crate) oam_read_block: bool,
    pub(crate) oam_write_block: bool,
    vram_read_block: bool,
    vram_write_block: bool,

    /// The current screen been render.
    /// Each pixel is a shade of gray, from 0 to 3
    pub screen: Vec<u32>,
    /// sprites that will be rendered in the next mode 3 scanline
    pub sprite_buffer: Vec<Sprite>,
    /// the length of the `sprite_buffer`
    pub sprite_buffer_len: u8,
    /// Window Internal Line Counter
    pub wyc: u8,

    /// FF40: LCD Control Register
    pub lcdc: u8,
    /// FF41: LCD Status Register
    /// Bit 6 - LYC=LY STAT Interrupt source
    /// Bit 5 - Mode 2 OAM STAT Interrupt source
    /// Bit 4 - Mode 1 VBlank STAT Interrupt source
    /// Bit 3 - Mode 0 HBlank STAT Interrupt source
    /// Bit 2 - LYC=LY Flag
    /// Bit 1-0 - Mode Flag
    pub stat: u8,
    /// FF42: Scroll Y Register
    pub scy: u8,
    /// FF43: Scroll x Register
    pub scx: u8,
    /// FF44: LCDC Y-Coordinate
    pub ly: u8,
    /// FF45: LY Compare
    pub lyc: u8,
    /// FF47: BG & Window Palette Data
    pub bgp: u8,
    /// FF48:
    pub obp0: u8,
    /// FF49:
    pub obp1: u8,
    /// FF4A: Window Y Position
    pub wy: u8,
    /// FF4B: Window X Position
    pub wx: u8,

    pub state: PpuState,
    /// When making the LY==LYC comparison, uses this value instead of ly to control the comparison
    /// timing. This is 0xFF if this will not update the stat.
    ly_for_compare: u8,

    stat_signal: bool,
    ly_compare_signal: bool,
    /// use this value instead of the current stat mode when controlling the stat interrupt signal,
    /// to control the timing. 0xff means that this will not trigger a interrupt.
    stat_mode_for_interrupt: u8,
    /// Current clock cycle
    pub(crate) ticks: usize,
    /// Next clock cycle where the PPU will be updated
    pub next_ticks: usize,
    /// The clock count in which the current scanline has started.
    pub line_start_ticks: usize,

    pub background_fifo: PixelFifo,
    pub sprite_fifo: PixelFifo,

    // pixel fetcher
    fetcher_step: u8,
    /// the tile x position that the pixel fetcher is in
    fetcher_x: u8,
    fetch_tile_number: u8,
    fetch_tile_data_low: u8,
    fetch_tile_data_high: u8,

    sprite_tile_address: u16,
    sprite_tile_data_low: u8,
    sprite_tile_data_high: u8,

    reach_window: bool,
    is_in_window: bool,

    /// Sprites at 0 cause a extra delay in the sprite fetching.
    sprite_at_0_penalty: u8,

    /// The x position of the next screen pixel to be draw in the current scanline
    pub screen_x: u8,
    /// The x position in the current scanline, from -(8 + scx%8) to 160. Negative values
    /// (represented by positives between 241 and 255) are use for detecting sprites that starts
    /// to the left of the screen, and for discarding pixels for scrolling.
    scanline_x: u8,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Copy, Debug)]
pub struct Sprite {
    pub sx: u8,
    pub sy: u8,
    pub tile: u8,
    pub flags: u8,
}

#[derive(Debug, PartialEq, Eq)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug, Ord, PartialOrd, Eq)]
pub enum PixelTransferPhase {
    TurnOnPixelTransfer,
    TurnOnDelay,
    FinishTurnOn,
    StartTransfer,
    ClearQueue,
    WindowActivationCheck,
    WindowActivation,
    SpriteHandling,
    SpriteFetching,
    BackgroundFetching,
    FirstSpritePenaltyCheck,
    FirstSpritePenalty,
    FirstPixelFetching,
    SecondPixelFetching,
    LowSpriteDataSetting,
    HighSpriteDataSetting,
    SpritePushing,
    EndTransfer,
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug, Ord, PartialOrd, Eq)]
pub enum HorizontalBlankPhase {
    TurnOnHBlank,
    StartHBlank,
    StartHBlankDelay,
    ClockLine,
    BlockOam,
    ElapsedTickCalculation,
    ReachWindow,
    IncreaseLine,
    StartLine,
    EndHBlank,
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug, Ord, PartialOrd, Eq)]
pub enum VerticalBlankPhase {
    StartVBlank,
    IncreaseVBlankLine,
    FirstLineCheck,
    LycUpdate,
    InterruptCheck,
    StartLineReset,
    ProgressLineReset,
    LastVBlankLine,
    FinishLineReset,
    EmptyWait,
    EndVBlank,
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug, Ord, PartialOrd, Eq)]
pub enum OamSearchPhase {
    StartOamSearch,
    EndOamSearch,
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug, Ord, PartialOrd, Eq)]
pub enum PpuState {
    OamSearch(OamSearchPhase),
    PixelTransfer(PixelTransferPhase),
    HorizontalBlank(HorizontalBlankPhase),
    VerticalBlank(VerticalBlankPhase),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
pub struct PixelFifo {
    queue: Vec<u8>,
    /// next position to push
    head: u8,
    /// next position to pop
    tail: u8,
}

impl PixelFifo {
    fn new() -> Self {
        Self {
            queue: vec![0; 16],
            ..Default::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
    }

    fn push_background(&mut self, tile_low: u8, tile_high: u8) {
        for i in (0..8).rev() {
            let color = (((tile_high >> i) & 0x01) << 1) | ((tile_low >> i) & 0x01);
            debug_assert!(color < 4);
            let pixel = color;
            self.queue[self.head as usize] = pixel;
            self.head = (self.head + 1) % self.queue.len() as u8;
            debug_assert_ne!(self.head, self.tail);
        }
    }

    fn push_sprite(
        &mut self,
        tile_low: u8,
        tile_high: u8,
        palette: bool,
        background_priority: bool,
    ) {
        let pixel = |x| {
            let color: u8 = (((tile_high >> x) & 0x01) << 1) | ((tile_low >> x) & 0x01);
            debug_assert!(color < 4);

            color | ((background_priority as u8) << 3) | ((palette as u8) << 4)
        };

        let mut cursor = self.tail;
        let mut x = 8u8;
        // overwrite pixels in fifo, but only if 0
        while cursor != self.head && x != 0 {
            x -= 1;
            let color = self.queue[cursor as usize] & 0b11;
            if color == 0 {
                self.queue[cursor as usize] = pixel(x);
            }
            cursor = (cursor + 1) % self.queue.len() as u8;
        }
        // write remained
        for x in (0..x).rev() {
            self.queue[self.head as usize] = pixel(x);
            self.head = (self.head + 1) % self.queue.len() as u8;
            debug_assert_ne!(self.head, self.tail);
        }
    }

    fn pop_front(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }
        let v = self.queue[self.tail as usize];
        self.tail = (self.tail + 1) % self.queue.len() as u8;
        Some(v)
    }
}

impl MemoryArea for PixelProcessingUnit {
    fn read(&self, address: usize) -> Option<u8> {
        let value = match address {
            0x8000..=0x9FFF if self.vram_read_block => 0xFF,
            0xFE00..=0xFE9F if self.dma_block_oam || self.oam_read_block => 0xFF,
            0x8000..=0x9FFF => self.vram[address - 0x8000],
            0xFE00..=0xFE9F => self.oam[address - 0xFE00],
            0xFF40 => self.lcdc,
            0xFF41 => self.stat | 0x80,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF46 => self.dma,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => return None,
        };
        Some(value)
    }

    fn write(&mut self, address: usize, value: u8) -> bool {
        match address {
            0x8000..=0x9FFF if self.vram_write_block => (),
            0xFE00..=0xFE9F if self.oam_write_block => (),
            0x8000..=0x9FFF => self.vram[address as usize - 0x8000] = value,
            0xFE00..=0xFE9F => self.oam[address as usize - 0xFE00] = value,
            0xFF46 => self.start_dma(value),
            0xFF40 => {
                if value & 0x80 != self.lcdc & 0x80 {
                    if value & 0x80 == 0 {
                        // disable ppu
                        self.ly = 0;
                        self.line_start_ticks = 0;
                        // set to mode 0
                        self.stat &= !0b11;
                        self.state = HorizontalBlank(TurnOnHBlank);
                    } else {
                        // enable ppu
                        debug_assert_eq!(self.ly, 0);
                        self.ly_for_compare = 0;
                        debug_assert_eq!(self.stat & 0b11, 0b00);
                        self.next_ticks = self.ticks;
                    }
                }
                self.lcdc = value
            }
            0xFF41 => self.stat = 0x80 | (value & !0b111) | (self.stat & 0b111),
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {} // ly is read only
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => return false,
        }
        true
    }
}

impl PixelProcessingUnit {
    pub fn new() -> Self {
        Self {
            oam_start_clock_count: 0,
            oam_corruption: None,
            vram: vec![0; 0x2000],
            oam: vec![0; 0xA0],
            dma: 0xFF,
            dma_started: 0,
            dma_running: false,
            dma_block_oam: false,
            oam_read_block: false,
            oam_write_block: false,
            vram_read_block: false,
            vram_write_block: false,
            screen: vec![0; 0x5A00],
            sprite_buffer: vec![Sprite::default(); 10],
            sprite_buffer_len: 0,
            wyc: 0,
            lcdc: 0x91,
            stat: 0x05,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xfc,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            state: VerticalBlank(EndVBlank),
            ly_for_compare: 0,

            ticks: 0,
            next_ticks: 23_440_377,
            line_start_ticks: 23_435_361,

            background_fifo: PixelFifo::new(),
            sprite_fifo: PixelFifo::new(),

            fetcher_step: 0x03,
            fetcher_x: 0x14,
            fetch_tile_number: 0,
            fetch_tile_data_low: 0,
            fetch_tile_data_high: 0,

            sprite_tile_address: 0,
            sprite_tile_data_low: 0,
            sprite_tile_data_high: 0,

            reach_window: true,
            is_in_window: false,
            stat_signal: false,
            ly_compare_signal: false,
            stat_mode_for_interrupt: 1,

            sprite_at_0_penalty: 0,

            screen_x: 0xa0,
            scanline_x: 0x00,
        }
    }

    fn search_objects(&mut self) {
        self.sprite_buffer_len = 0;
        let sprite_height = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        for i in 0..40 {
            let i = i as usize * 4;
            let data = &self.oam[i..i + 4];
            let sy = data[0];
            let sx = data[1];
            let t = data[2];
            let flags = data[3];

            if self.ly as u16 + 16 >= sy as u16 && self.ly as u16 + 16 < sy as u16 + sprite_height {
                self.sprite_buffer[self.sprite_buffer_len as usize] = Sprite {
                    sy,
                    sx,
                    tile: t,
                    flags,
                };
                self.sprite_buffer_len += 1;
            }
            if self.sprite_buffer_len == 10 {
                break;
            }
        }
        // sort buffer by priority, in increasing order
        // lower x position, has greater priority
        self.sprite_buffer[0..self.sprite_buffer_len as usize].reverse();
        self.sprite_buffer[0..self.sprite_buffer_len as usize].sort_by_key(|x| !x.sx);
    }

    pub fn start_dma(&mut self, value: u8) {
        self.dma = value;
        self.dma_started = self.ticks - 4;
        if self.dma_running {
            // HACK: if a DMA requested was make right before this one, this dma_started
            // rewritten would cancel the oam_block of that DMA. To fix this, I will hackly
            // block the oam here, but this will block the oam 4 cycles early, but I think
            // this will not be observable.
            self.dma_block_oam = true;
        }
        self.dma_running = true;
    }

    pub fn machine_cycle(&mut self, ticks: usize) -> (bool, bool) {
        self.ticks += ticks;

        // Most of the ppu behaviour is based on the LIJI32/SameBoy including all of the timing,
        // and most of the implementation.
        if self.lcdc & 0x80 == 0 {
            // ppu is disabled
            self.next_ticks = self.ticks;
            return (false, false);
        }

        let mut stat_interrupt = false;
        let mut vblank_interrupt = false;

        self.update_stat(&mut stat_interrupt);

        while self.next_ticks < self.ticks {
            let (clocks, state) =
                self.handle_state_transition(&mut vblank_interrupt, &mut stat_interrupt);
            self.next_ticks += clocks;
            self.state = state;
        }

        self.handle_oam_corruption();

        (vblank_interrupt, stat_interrupt)
    }

    fn handle_state_transition(
        &mut self,
        vblank_interrupt: &mut bool,
        stat_interrupt: &mut bool,
    ) -> (usize, PpuState) {
        match self.state {
            HorizontalBlank(TurnOnHBlank) => {
                self.ly = 0;

                self.set_stat_mode(0);
                self.stat_mode_for_interrupt = 0;

                self.reach_window = false;
                self.screen_x = 0;

                self.oam_read_block = false;
                self.oam_write_block = false;
                self.vram_read_block = false;
                self.vram_write_block = false;

                // In the SameBoy, there is a delay of 1 T-cycle between this state and the
                // next. I changed it to 0 because in SameBoy the PPU activation happens 11
                // T-cycles after the opcode read, but in my emulator the delay is 12 T-cycles
                // (3 M-cycles). SameBoy must have a better write timing?
                (0, HorizontalBlank(ClockLine))
            }
            // 1
            HorizontalBlank(ClockLine) => {
                self.line_start_ticks = self.next_ticks - 8;
                self.wyc = 0xff;

                (76, HorizontalBlank(BlockOam))
            }
            // 77
            HorizontalBlank(BlockOam) => {
                self.oam_write_block = true;

                (2, PixelTransfer(TurnOnPixelTransfer))
            }
            // 79
            PixelTransfer(TurnOnPixelTransfer) => {
                self.oam_read_block = true;
                self.oam_write_block = true;
                self.vram_read_block = true;
                self.vram_write_block = true;

                self.set_stat_mode(3);
                self.stat_mode_for_interrupt = 3;

                (2, PixelTransfer(TurnOnDelay))
            }
            // 81
            PixelTransfer(TurnOnDelay) => (3, PixelTransfer(FinishTurnOn)),
            // 84
            PixelTransfer(FinishTurnOn) => (0, PixelTransfer(ClearQueue)),

            HorizontalBlank(StartLine) => {
                self.line_start_ticks = self.next_ticks;
                self.screen_x = 0;

                (3, HorizontalBlank(EndHBlank))
            }
            // 3
            HorizontalBlank(EndHBlank) => {
                self.oam_read_block = true;
                self.set_stat_mode(0);

                if self.ly == 0 {
                    self.ly_for_compare = 0;
                    self.stat_mode_for_interrupt = 0xff;
                } else {
                    self.ly_for_compare = 0xFF;
                    self.stat_mode_for_interrupt = 2;
                }

                self.update_stat(stat_interrupt);

                (1, OamSearch(StartOamSearch))
            }
            // 4
            OamSearch(StartOamSearch) => {
                self.oam_write_block = true;

                self.ly_for_compare = self.ly;

                self.set_stat_mode(2);
                self.oam_start_clock_count = self.ticks;
                self.stat_mode_for_interrupt = 2;
                self.update_stat(stat_interrupt);
                self.stat_mode_for_interrupt = 0xff;
                self.update_stat(stat_interrupt);

                self.search_objects();

                (76, OamSearch(EndOamSearch))
            }
            // 80
            OamSearch(EndOamSearch) => {
                self.oam_read_block = true;
                self.oam_write_block = false;
                self.vram_read_block = true;
                self.vram_write_block = false;

                (4, PixelTransfer(StartTransfer))
            }
            // 84
            PixelTransfer(StartTransfer) => {
                debug_assert_eq!(self.next_ticks - self.line_start_ticks, 84);
                self.set_stat_mode(3);
                self.stat_mode_for_interrupt = 3;
                self.update_stat(stat_interrupt);

                self.oam_read_block = true;
                self.oam_write_block = true;
                self.vram_read_block = true;
                self.vram_write_block = true;

                (5, PixelTransfer(ClearQueue))
            }

            PixelTransfer(ClearQueue) => {
                self.background_fifo.clear();
                self.sprite_fifo.clear();

                // Fill background FIFO with 8 dummy pixels
                self.background_fifo.push_background(0x00, 0x00);

                self.fetcher_step = 0;
                self.fetcher_x = 0;
                if self.wy == self.ly {
                    self.reach_window = true;
                }
                self.is_in_window = false;
                self.scanline_x = -((self.scx % 8 + 8) as i8) as u8;
                self.sprite_at_0_penalty = (self.scx % 8).min(5);

                (0, PixelTransfer(WindowActivationCheck))
            }
            // Loop for every line from 0 to 144
            PixelTransfer(WindowActivationCheck) => {
                let window_enabled = self.lcdc & 0x20 != 0;
                if self.is_in_window || !self.reach_window || !window_enabled {
                    return (0, PixelTransfer(SpriteHandling));
                }

                let mut should_active = false;

                if self.wx == 0 {
                    let cmp = [-7i8, -9, -10, -11, -12, -13, -14, -14];
                    if self.scanline_x == cmp[(self.scx % 8) as usize] as u8 {
                        should_active = true;
                    }
                } else if self.wx < 166 {
                    if self.wx == self.scanline_x.wrapping_add(7) {
                        should_active = true;
                    } else if self.wx == self.scanline_x.wrapping_add(6) {
                        // TODO: && !wx_just_changed
                        should_active = true;
                        if self.screen_x > 0 {
                            self.screen_x -= 1;
                        }
                    }
                }

                if should_active {
                    // wrapping add, because wyc starts at -1
                    self.wyc = self.wyc.wrapping_add(1);
                    if self.wx == 0 && self.scx % 8 != 0 {
                        // wait 1
                        return (1, PixelTransfer(WindowActivation));
                    }
                    return (0, PixelTransfer(WindowActivation));
                } else if self.wx == 166 && self.wx == self.scanline_x + 7 {
                    self.wyc += 1;
                }

                (0, PixelTransfer(SpriteHandling))
            }
            PixelTransfer(WindowActivation) => {
                self.is_in_window = true;
                self.fetcher_x = 0;
                self.fetcher_step = 0;
                self.background_fifo.clear();

                (0, PixelTransfer(SpriteHandling))
            }
            PixelTransfer(SpriteHandling) => {
                // Discard already handled sprites
                // TODO: discover why this is necessary (blindly copied from SameBoy)
                while self.sprite_buffer_len > 0
                    && (self.scanline_x < 160 || self.scanline_x >= (-8i8) as u8)
                    && self.sprite_buffer[self.sprite_buffer_len as usize - 1].sx
                        < self.scanline_x.wrapping_add(8)
                {
                    self.sprite_buffer_len -= 1;
                }

                (0, PixelTransfer(SpriteFetching))
            }
            // While there are sprites to be fetched
            PixelTransfer(SpriteFetching) => {
                let sprite_enabled = self.lcdc & 0x02 != 0;
                if self.sprite_buffer_len > 0
                    && sprite_enabled
                    && self.sprite_buffer[self.sprite_buffer_len as usize - 1].sx
                        == self.scanline_x.wrapping_add(8)
                {
                    (0, PixelTransfer(BackgroundFetching))
                } else {
                    (0, PixelTransfer(EndTransfer))
                }
            }
            // while there are background pixels or don't reach a fetcher step...
            PixelTransfer(BackgroundFetching) => {
                if self.background_fifo.is_empty() || self.fetcher_step < 5 {
                    self.tick_pixel_fetcher(self.ly);
                    (1, PixelTransfer(BackgroundFetching))
                } else {
                    (0, PixelTransfer(FirstSpritePenaltyCheck))
                }
            }
            PixelTransfer(FirstSpritePenaltyCheck) => {
                // TODO: handle extra penalty sprite at 0
                if self.sprite_at_0_penalty != 0
                    && self.sprite_buffer[self.sprite_buffer_len as usize - 1].sx == 0
                {
                    let penalty = self.sprite_at_0_penalty as usize;
                    self.sprite_at_0_penalty = 0;
                    return (penalty, PixelTransfer(FirstSpritePenalty));
                }

                (0, PixelTransfer(FirstPixelFetching))
            }
            PixelTransfer(FirstSpritePenalty) => (0, PixelTransfer(FirstPixelFetching)),
            PixelTransfer(FirstPixelFetching) => {
                self.tick_pixel_fetcher(self.ly);

                (1, PixelTransfer(SecondPixelFetching))
            }
            PixelTransfer(SecondPixelFetching) => {
                self.tick_pixel_fetcher(self.ly);
                self.sprite_tile_address = {
                    let tall = self.lcdc & 0x04 != 0;
                    let sprite = self.sprite_buffer[self.sprite_buffer_len as usize - 1];
                    let flip_y = sprite.flags & 0x40 != 0;

                    let height = if tall { 0xF } else { 0x7 };
                    let mut py = self.ly.wrapping_sub(sprite.sy) & height;
                    if flip_y {
                        py = (!py) & height;
                    }

                    let tile = if tall { sprite.tile & !1 } else { sprite.tile };
                    tile as u16 * 0x10 + py as u16 * 2
                };

                (2, PixelTransfer(LowSpriteDataSetting))
            }
            PixelTransfer(LowSpriteDataSetting) => {
                self.sprite_tile_data_low = self.vram[self.sprite_tile_address as usize];

                (2, PixelTransfer(HighSpriteDataSetting))
            }
            PixelTransfer(HighSpriteDataSetting) => {
                self.sprite_tile_data_high = self.vram[self.sprite_tile_address as usize + 1];

                (1, PixelTransfer(SpritePushing))
            }
            PixelTransfer(SpritePushing) => {
                let sprite = self.sprite_buffer[self.sprite_buffer_len as usize - 1];
                let flip_x = sprite.flags & 0x20 != 0;
                let tile_low = if flip_x {
                    self.sprite_tile_data_low.reverse_bits()
                } else {
                    self.sprite_tile_data_low
                };
                let tile_height = if flip_x {
                    self.sprite_tile_data_high.reverse_bits()
                } else {
                    self.sprite_tile_data_high
                };
                self.sprite_fifo.push_sprite(
                    tile_low,
                    tile_height,
                    sprite.flags & 0x10 != 0,
                    sprite.flags & 0x80 != 0,
                );
                self.sprite_buffer_len -= 1;

                // loop again
                (0, PixelTransfer(SpriteFetching))
            }
            PixelTransfer(EndTransfer) => {
                self.output_pixel();
                self.tick_pixel_fetcher(self.ly);

                debug_assert!(self.screen_x <= 160);
                if self.screen_x == 160 {
                    (0, HorizontalBlank(StartHBlank))
                } else {
                    (1, PixelTransfer(WindowActivationCheck))
                }
            }
            HorizontalBlank(StartHBlank) => {
                self.oam_read_block = false;
                self.oam_write_block = false;
                self.vram_read_block = false;
                self.vram_write_block = false;

                self.set_stat_mode(0);
                self.stat_mode_for_interrupt = 0;
                self.update_stat(stat_interrupt);

                (1, HorizontalBlank(StartHBlankDelay))
            }
            HorizontalBlank(StartHBlankDelay) => (2, HorizontalBlank(ElapsedTickCalculation)),
            HorizontalBlank(ElapsedTickCalculation) => {
                let elapsed = self.next_ticks - self.line_start_ticks;

                (454 - elapsed, HorizontalBlank(ReachWindow))
            }
            HorizontalBlank(ReachWindow) => {
                if self.lcdc & 0x20 != 0 && self.wy == self.ly {
                    self.reach_window = true;
                }

                (2, HorizontalBlank(IncreaseLine))
            }
            HorizontalBlank(IncreaseLine) => {
                self.ly += 1;
                if self.ly == 144 {
                    (0, VerticalBlank(StartVBlank))
                } else {
                    (0, HorizontalBlank(StartLine))
                }
            }
            VerticalBlank(StartVBlank) => {
                if self.ly == 153 {
                    return (0, VerticalBlank(LastVBlankLine));
                }
                self.ly_for_compare = 0xFF;
                self.update_stat(stat_interrupt);

                (2, VerticalBlank(InterruptCheck))
            }
            // 2
            VerticalBlank(InterruptCheck) => {
                if self.ly == 144 && !self.stat_signal && self.stat & 0x20 != 0 {
                    *stat_interrupt = true;
                }

                (2, VerticalBlank(LycUpdate))
            }
            // 4
            VerticalBlank(LycUpdate) => {
                self.ly_for_compare = self.ly;
                self.update_stat(stat_interrupt);

                (0, VerticalBlank(FirstLineCheck))
            }
            VerticalBlank(FirstLineCheck) => {
                if self.ly == 144 {
                    self.set_stat_mode(1);
                    *vblank_interrupt = true;
                    if !self.stat_signal && self.stat & 0x20 != 0 {
                        *stat_interrupt = true;
                    }
                    self.stat_mode_for_interrupt = 1;
                    self.update_stat(stat_interrupt);
                }

                (452, VerticalBlank(IncreaseVBlankLine))
            }
            VerticalBlank(IncreaseVBlankLine) => {
                self.ly += 1;

                (0, VerticalBlank(StartVBlank))
            }

            VerticalBlank(LastVBlankLine) => {
                self.ly = 153;
                self.ly_for_compare = 0xFF;
                self.update_stat(stat_interrupt);

                (6, VerticalBlank(StartLineReset))
            }
            // 6
            VerticalBlank(StartLineReset) => {
                self.ly = 0;
                self.ly_for_compare = 153;
                self.update_stat(stat_interrupt);

                (2, VerticalBlank(ProgressLineReset))
            }
            // 8
            VerticalBlank(ProgressLineReset) => {
                self.ly = 0;
                self.update_stat(stat_interrupt);

                (4, VerticalBlank(FinishLineReset))
            }
            // 12
            VerticalBlank(FinishLineReset) => {
                self.ly_for_compare = 0;
                self.update_stat(stat_interrupt);

                (12, VerticalBlank(EmptyWait))
            }
            // 24
            VerticalBlank(EmptyWait) => (432, VerticalBlank(EndVBlank)),
            // 0
            VerticalBlank(EndVBlank) => {
                self.ly = 0;
                self.reach_window = false;
                self.wyc = 0xff;

                (0, HorizontalBlank(StartLine))
            }
        }
    }

    fn handle_oam_corruption(&mut self) {
        let row = (self.ticks - self.oam_start_clock_count) as usize / 4;

        if self.stat & 0b11 != 2 {
            return self.oam_corruption = None;
        }

        if let Some(cause) = self.oam_corruption {
            match cause {
                Write | IncDec => self.handle_oam_write_corruption(row),
                Read => self.handle_oam_read_corruption(row),
                ReadWrite => self.handle_oam_read_write_corruption(row),
            };
        }
        self.oam_corruption = None;
    }

    fn handle_oam_read_write_corruption(&mut self, row: usize) {
        if row != 0 && row != 1 && row != 19 {
            let _rows = self.oam.chunks(8);

            let mut rows = self.oam.chunks_mut(8);

            let (second_row, first_row, current_row) = (
                rows.nth(row - 2).unwrap(),
                rows.next().unwrap(),
                rows.next().unwrap(),
            );

            let a = u16::from_le_bytes(second_row[0..2].as_ref().try_into().unwrap());
            let b = u16::from_le_bytes(first_row[0..2].as_ref().try_into().unwrap());
            let c = u16::from_le_bytes(current_row[0..2].as_ref().try_into().unwrap());
            let d = u16::from_le_bytes(first_row[4..6].as_ref().try_into().unwrap());

            let pattern = ((b & (a | c | d)) | (a & c & d)).to_le_bytes();
            first_row[0..2].clone_from_slice(pattern.as_slice());
            second_row.clone_from_slice(first_row);
            current_row.clone_from_slice(first_row);
        }
        self.handle_oam_read_corruption(row);
    }

    fn handle_oam_read_corruption(&mut self, row: usize) {
        self.handle_oam_pattern_corruption(|a, b, c| b | (a & c), row);
    }

    fn handle_oam_write_corruption(&mut self, row: usize) {
        self.handle_oam_pattern_corruption(|a, b, c| ((a ^ c) & (b ^ c)) ^ c, row);
    }

    fn handle_oam_pattern_corruption(&mut self, pattern: fn(u16, u16, u16) -> u16, row: usize) {
        if row == 0 {
            return;
        }
        let mut rows = self.oam.chunks_mut(8);

        let (previous_row, current_row) = (rows.nth(row - 1).unwrap(), rows.next().unwrap());

        let a = u16::from_le_bytes(current_row[0..2].as_ref().try_into().unwrap());
        let b = u16::from_le_bytes(previous_row[0..2].as_ref().try_into().unwrap());
        let c = u16::from_le_bytes(previous_row[4..6].as_ref().try_into().unwrap());

        let pattern = pattern(a, b, c).to_le_bytes();
        current_row[0..2].clone_from_slice(pattern.as_slice());
        current_row[2..].clone_from_slice(&previous_row[2..]);
    }

    fn set_stat_mode(&mut self, mode: u8) {
        self.stat = (self.stat & !0b11) | mode;
    }

    fn update_stat(&mut self, stat_interrupt: &mut bool) {
        let stat_mode = self.stat_mode_for_interrupt;
        let mut stat_line = false;

        match stat_mode {
            0 => stat_line |= self.stat & 0x08 != 0,
            1 => {
                // VBlank also trigger OAM STAT interrupt
                stat_line |= self.stat & 0x30 != 0;
            }
            2 => stat_line |= self.stat & 0x20 != 0,
            3 => {}
            255 => {}
            4..=254 => unreachable!(),
        }

        // LY==LYC
        self.stat &= !0x04;
        if self.ly_for_compare == self.lyc {
            self.ly_compare_signal = true;
            // STAT Coincident Flag
            self.stat |= 0x04;
        } else if self.ly_for_compare != 0xff {
            self.ly_compare_signal = false;
        }
        // LY == LYC STAT Interrupt
        stat_line |= (self.stat & (1 << 6) != 0) && self.ly_compare_signal;

        // on rising edge
        if !self.stat_signal && stat_line {
            *stat_interrupt = true;
        }

        self.stat_signal = stat_line;
    }

    fn tick_pixel_fetcher(&mut self, ly: u8) {
        let is_in_window = self.is_in_window;

        let fetch_tile_address =
            |ppu: &mut PixelProcessingUnit, is_in_window: bool, ly: u8| -> u16 {
                let mut tile = ppu.fetch_tile_number as u16;
                if ppu.lcdc & 0x10 == 0 {
                    tile += 0x100;
                    if tile >= 0x180 {
                        tile -= 0x100;
                    }
                }
                let address = tile * 0x10 + 0x8000;
                let offset = if is_in_window {
                    2 * (ppu.wyc as u16 % 8)
                } else {
                    2 * (ly.wrapping_add(ppu.scy) % 8) as u16
                };

                address + offset
            };

        let push_to_fifo = |ppu: &mut PixelProcessingUnit| {
            if ppu.background_fifo.is_empty() {
                let low = ppu.fetch_tile_data_low;
                let high = ppu.fetch_tile_data_high;
                ppu.background_fifo.push_background(low, high);
                ppu.fetcher_step = 0;
            }
        };

        match self.fetcher_step {
            0 => {}
            // fetch tile number
            1 => {
                let tile_map = if !is_in_window {
                    if self.lcdc & 0x08 != 0 {
                        0x9C00
                    } else {
                        0x9800
                    }
                } else if self.lcdc & 0x40 != 0 {
                    0x9C00
                } else {
                    0x9800
                };

                let tx = if is_in_window {
                    self.fetcher_x
                } else {
                    ((self.scx.wrapping_add(self.scanline_x).wrapping_add(8)) / 8) & 0x1f
                };
                let ty = if is_in_window {
                    self.wyc / 8
                } else {
                    ly.wrapping_add(self.scy) / 8
                };

                let offset = (32 * ty as u16 + tx as u16) & 0x03ff;
                self.fetch_tile_number = self.vram[(tile_map + offset) as usize - 0x8000];
            }
            2 => {}
            // fetch tile data (low)
            3 => {
                let fetch_tile_address = fetch_tile_address(self, is_in_window, ly);
                self.fetch_tile_data_low = self.vram[fetch_tile_address as usize - 0x8000];
            }
            4 => {}
            // fetch tile data (high)
            5 => {
                let fetch_tile_address = fetch_tile_address(self, is_in_window, ly);
                self.fetch_tile_data_high = self.vram[fetch_tile_address as usize + 1 - 0x8000];
                if self.is_in_window {
                    self.fetcher_x += 1;
                }

                self.fetcher_step += 1;
                push_to_fifo(self);
                // the step may change to 0, and must not be increase at the end of this function
                return;
            }
            // push to fifo
            6 | 7 => {
                push_to_fifo(self);
                // the step may change to 0, and must not be increase at the end of this function
                return;
            }
            8..=255 => unreachable!(),
        }
        self.fetcher_step += 1;
    }

    fn output_pixel(&mut self) {
        if let Some(pixel) = self.background_fifo.pop_front() {
            let sprite_pixel = self.sprite_fifo.pop_front();

            // scanline_x values greater or equal than 160 are interpreted as negative (for scrolling)
            // or are out of bounds.
            if self.scanline_x >= 160 {
                // Discart the pixel. Used for scrolling the background.
                self.scanline_x = self.scanline_x.wrapping_add(1);
                return;
            }

            let i = (self.ly as usize) * 160 + self.screen_x as usize;
            let background_enable = self.lcdc & 0x01 != 0;
            let bcolor = if background_enable { pixel & 0b11 } else { 0 };

            // background color, with pallete applied
            let palette = self.bgp;
            let mut color = (palette >> (bcolor * 2)) & 0b11;

            if let Some(sprite_pixel) = sprite_pixel {
                let scolor = sprite_pixel & 0b11;
                let background_priority = (sprite_pixel >> 3) & 0x01 != 0;
                if scolor == 0 || background_priority && bcolor != 0 {
                    // use background color
                } else {
                    // use sprite color
                    let palette = (sprite_pixel >> 4) & 0x1;
                    let palette = [self.obp0, self.obp1][palette as usize];
                    color = (palette >> (scolor * 2)) & 0b11;
                }
            }
            debug_assert!(color < 4);
            self.screen[i] = match color {
                0 => WHITE,
                1 => LIGHT_GRAY,
                2 => DARK_GRAY,
                3 => BLACK,
                _ => unreachable!(),
            }
            .into();
            self.screen_x += 1;
            self.scanline_x += 1;
        }
    }
}

impl From<Color> for u32 {
    fn from(color: Color) -> Self {
        let Color { a, r, g, b } = color;
        u32::from_be_bytes([a, r, g, b])
    }
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
