use crate::interrupt::InterruptHandler;
use crate::interrupt::InterruptId::{Input, Serial, Stat, Timing, VBlank};
use crate::joypad::Joypad;
use crate::ppu::DmaState::Executing;
use crate::ppu::PpuState::ModeChange;
use crate::ppu::RenderCycle::{Normal, StatTrigger};
use crate::ppu::{PixelProcessingUnit, PpuMode};
use crate::timer::Timer;
use minifb::{Scale, ScaleMode, Window, WindowOptions};
use std::any::{Any, TypeId};
use OamCorruptionCause::IncDec;
use PpuMode::{OamSearch, VerticalBlank};

use crate::serial::LinkCable;

macro_rules! set_memory {
    {$mm:ident, $($addr:literal: $val:literal,)*} => {
        $(
            $mm.write_without_cycle($addr as u16, $val);
        )*
    }
}

#[derive(Debug)]
pub enum OamCorruptionCause {
    IncDec,
    Read,
    Write,
    ReadWrite,
}

pub struct MemoryMap {
    pub memory: Vec<u8>,
    pub interrupt_handler: InterruptHandler,
    pub ppu: PixelProcessingUnit,
    serial: LinkCable,
    timer: Timer,
    joypad: Joypad,
    rom_size: usize,
    pub cycles: u16,
    dma_progress: u8,
    oam_corruption: Option<OamCorruptionCause>,
    pub window: Option<Window>,
}

impl MemoryMap {
    pub fn new(rom: &Vec<u8>, rom_name: &str, headless: bool) -> MemoryMap {
        let ppu = PixelProcessingUnit::new();
        let joypad = Joypad::new();
        let interrupt_handler = InterruptHandler::new();
        let timer = Timer::new();
        let rom_size = rom.len() as usize;
        let memory = vec![0; 0x10000];
        let micro_ops = 0;
        let dma_progress = 0;
        let oam_corruption = None;
        let serial = LinkCable::new();
        let window = if headless {
            None
        } else {
            Some(Window::new(
                format!("{} - ESC to exit", rom_name).as_str(),
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
                },
            ).unwrap())
        };
        let mem = MemoryMap {
            joypad,
            ppu,
            interrupt_handler,
            timer,
            memory,
            rom_size,
            cycles: micro_ops,
            dma_progress,
            oam_corruption,
            serial,
            window,
        };
        MemoryMap::init_memory(mem, rom)
    }

    fn in_oam<T: 'static + Into<usize> + Copy>(&self, address: T) -> bool {
        let translated_address = if address.type_id() == TypeId::of::<u8>() {
            address.into() + 0xFF00
        } else {
            address.into()
        };
        // TODO: Figure out if the OAM check should always be in this range
        (0xFE00_usize..=0xFEFF_usize).contains(&translated_address)
    }

    pub fn trigger_oam_inc_dec_corruption<T: 'static + Into<usize> + Copy>(&mut self, address: T) {
        if !self.in_oam(address) {
            return;
        }
        self.ppu.oam_corruption = match self.ppu.oam_corruption {
            None => Some(IncDec),
            _ => panic!(),
        }
    }

    pub fn read<T: 'static + Into<usize> + Copy>(&mut self, address: T) -> u8 {
        let value = self.read_without_cycle(address);
        self.cycle();
        value
    }

    pub fn write<Address: 'static + Into<usize> + Copy, Value: Into<u8> + Copy>(
        &mut self,
        address: Address,
        value: Value,
    ) {
        self.write_without_cycle(address, value.into());
        self.cycle();
    }

    pub fn read_without_cycle<T: 'static + Into<usize> + Copy>(&mut self, address: T) -> u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory(address.into()));
        let translated_address = if address.type_id() == TypeId::of::<u8>() {
            address.into() + 0xFF00
        } else {
            address.into()
        };

        self.ppu
            .read(translated_address)
            .or_else(|| self.interrupt_handler.read(translated_address))
            .or_else(|| self.timer.read(translated_address))
            .or_else(|| self.joypad.read(translated_address))
            .or_else(|| self.serial.read(translated_address))
            .unwrap_or(self.memory[translated_address])
    }

    fn write_without_cycle<T: 'static + Into<usize> + Copy>(&mut self, address: T, value: u8) {
        //println!("Writing address {}", address.into());
        let translated_address = if address.type_id() == TypeId::of::<u8>() {
            address.into() + 0xFF00
        } else {
            address.into()
        };
        if !(self.ppu.write(translated_address, value)
            || self.timer.write(translated_address, value)
            || self.interrupt_handler.write(translated_address, value)
            || self.joypad.write(translated_address, value)
            || self.serial.write(translated_address, value))
            && (translated_address >= self.rom_size)
        {
            self.memory[translated_address] = value
        }
    }

    pub fn cycle(&mut self) {
        self.cycles += 1;
        self.dma_transfer();
        self.machine_cycle();
    }

    fn dma_transfer(&mut self) {
        if let Executing(n) = self.ppu.dma {
            while self.dma_progress < n {
                self.ppu.oam[self.dma_progress as usize] = self
                    .read_without_cycle(self.ppu.dma_offset * 0x100 + self.dma_progress as usize);
                self.dma_progress += 1;
            }
            if self.dma_progress as usize == self.ppu.oam.len() {
                self.dma_progress = 0;
            }
        }
    }

    fn machine_cycle(&mut self) {
        match self.ppu.machine_cycle() {
            StatTrigger(ModeChange(_, VerticalBlank)) => {
                self.interrupt_handler.set(VBlank);
                self.interrupt_handler.set(Stat);
            }
            Normal(ModeChange(_, VerticalBlank)) => self.interrupt_handler.set(VBlank),
            Normal(ModeChange(VerticalBlank, OamSearch)) => self.update_screen(),
            StatTrigger(state) => {
                if let ModeChange(_, VerticalBlank) = state {
                    self.update_screen()
                }
                self.interrupt_handler.set(Stat)
            }
            _ => (),
        };

        if self.timer.machine_cycle() {
            self.interrupt_handler.set(Timing)
        };

        if self.serial.machine_cycle() {
            self.interrupt_handler.set(Serial)
        };

        if self.window.as_ref().map(|window| self.joypad.machine_cycle(window)).unwrap_or(false) {
            self.interrupt_handler.set(Input)
        }

        self.oam_corruption = None;
    }

    fn update_screen(&mut self) {
        self.window.as_mut().map(|window| window.update_with_buffer(&self.ppu.pixels, 160, 144).unwrap());
    }

    fn init_memory(mut mem: MemoryMap, rom: &[u8]) -> MemoryMap {
        for (index, value) in rom.iter().enumerate() {
            mem.memory[index] = *value
        }

        set_memory! {
            mem,
            0xFF05: 0x0,
            0xFF06: 0x0,
            0xFF07: 0x0,
            0xFF10: 0x80,
            0xFF11: 0xBF,
            0xFF12: 0xF3,
            0xFF14: 0xBF,
            0xFF16: 0x3F,
            0xFF16: 0x3F,
            0xFF17: 0x0,
            0xFF19: 0xBF,
            0xFF1A: 0x7F,
            0xFF1B: 0xFF,
            0xFF1C: 0x9F,
            0xFF1E: 0xFF,
            0xFF20: 0xFF,
            0xFF21: 0x0,
            0xFF22: 0x0,
            0xFF23: 0xBF,
            0xFF24: 0x77,
            0xFF25: 0xF3,
            0xFF26: 0xF1,
            0xFF40: 0x91,
            0xFF42: 0x0,
            0xFF43: 0x0,
            0xFF45: 0x0,
            0xFF47: 0xFC,
            0xFF48: 0xFF,
            0xFF49: 0xFF,
            0xFF4A: 0x0,
            0xFF4B: 0x0,
            0xFF00: 0xFF,
        }

        mem
    }
}
