use crate::interrupt::InterruptHandler;
use crate::interrupt::InterruptId::{Input, Serial, Stat, Timing, VBlank};
use crate::joypad::Joypad;
use crate::mmu::OamCorruptionCause::{IncDec, Read, ReadWrite, Write};
use crate::ppu::PixelProcessingUnit;
use crate::timer::Timer;
use minifb::{Scale, ScaleMode, Window, WindowOptions};
use std::any::{Any, TypeId};
use std::fs::read;

use crate::serial::LinkCable;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OamCorruptionCause {
    IncDec,
    Read,
    Write,
    ReadWrite,
}

pub trait MemoryArea {
    fn read(&self, address: usize) -> Option<u8>;
    fn write(&mut self, address: usize, value: u8) -> bool;
}

pub struct MemoryManagementUnit {
    pub boot_rom: Option<Vec<u8>>,
    pub memory: Vec<u8>,
    pub interrupt_handler: InterruptHandler,
    pub ppu: PixelProcessingUnit,
    serial: LinkCable,
    timer: Timer,
    joypad: Joypad,
    rom_size: usize,
    pub cycles: u16,
    pub dma: u8,
    pub window: Option<Window>,
}

impl MemoryManagementUnit {
    pub fn new(
        rom: &Vec<u8>,
        rom_name: &str,
        headless: bool,
        boot_rom: Option<String>,
    ) -> MemoryManagementUnit {
        let ppu = PixelProcessingUnit::new();
        let joypad = Joypad::new();
        let interrupt_handler = InterruptHandler::new();
        let timer = Timer::new(boot_rom.is_some());
        let rom_size = rom.len() as usize;
        let memory = vec![0; 0x10000];
        let micro_ops = 0;

        let serial = LinkCable::new();
        let boot = boot_rom.map(read).map(|f| f.expect("Boot ROM not found"));
        let window = if headless {
            None
        } else {
            Some(
                Window::new(
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
                )
                .unwrap(),
            )
        };
        let mem = MemoryManagementUnit {
            dma: 0xFF,
            joypad,
            ppu,
            interrupt_handler,
            timer,
            memory,
            rom_size,
            cycles: micro_ops,
            serial,
            window,
            boot_rom: boot,
        };
        MemoryManagementUnit::init_memory(mem, rom)
    }

    fn in_echo(&self, address: usize) -> bool {
        (0xE000..0xFDFF).contains(&address)
    }

    fn in_oam(&self, address: usize) -> bool {
        (0xFE00_usize..=0xFEFF_usize).contains(&address)
    }

    pub fn corrupt_oam<T: 'static + Into<usize> + Copy>(&mut self, address: T) -> bool {
        if !self.in_oam(address.into()) {
            false
        } else {
            self.ppu.oam_corruption = Some(IncDec);
            true
        }
    }

    pub fn read<T: 'static + Into<usize> + Copy>(&mut self, address: T) -> u8 {
        let translated_address = if address.type_id() == TypeId::of::<u8>() {
            address.into() + 0xFF00
        } else {
            address.into()
        };

        if self.boot_rom.is_some() && translated_address < 0x100 {
            let value = self.boot_rom.as_ref().unwrap()[translated_address];
            self.cycle();
            return value;
        }

        if self.in_echo(translated_address) {
            return self.read(translated_address - 0x1000);
        }

        self.ppu.oam_corruption = match (self.in_oam(translated_address), self.ppu.oam_read_block, self.ppu.oam_corruption) {
            (true, true, None) => Some(Read),
            (true, true, Some(IncDec)) => Some(ReadWrite),
            (true, true, _) => unreachable!(),
            _ => None
        };

        let value = self.internal_read(translated_address);
        self.cycle();
        value
    }

    pub fn write<Address: 'static + Into<usize> + Copy, Value: Into<u8> + Copy>(
        &mut self,
        address: Address,
        value: Value,
    ) {
        let translated_address = if address.type_id() == TypeId::of::<u8>() {
            address.into() + 0xFF00
        } else {
            address.into()
        };

        if translated_address == 0xFF50 && self.boot_rom.is_some() && value.into() == 1 {
            self.boot_rom = None;
            self.cycle();
            return;
        }

        if self.in_echo(translated_address) {
            return self.write(translated_address - 0x1000, value);
        }

        self.ppu.oam_corruption = match (self.in_oam(translated_address), self.ppu.oam_read_block, self.ppu.oam_corruption) {
            (true, true, None | Some(IncDec)) => Some(Write),
            (true, true, _) => unreachable!(),
            _ => None
        };

        self.internal_write(translated_address, value.into());
        self.cycle();
    }

    pub fn internal_read(&self, translated_address: usize) -> u8 {
        self.ppu
            .read(translated_address)
            .or_else(|| self.interrupt_handler.read(translated_address))
            .or_else(|| self.timer.read(translated_address))
            .or_else(|| self.joypad.read(translated_address))
            .or_else(|| self.serial.read(translated_address))
            .unwrap_or(self.memory[translated_address])
    }

    fn internal_write(&mut self, translated_address: usize, value: u8) {
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

    pub fn dma_transfer(&mut self) {
        if !self.ppu.dma_running {
            return;
        }
        let elapsed = self.ppu.clock_count.wrapping_sub(self.ppu.dma_started);
        if elapsed < 8 {
            return;
        }

        self.ppu.dma_block_oam = true;

        // 8 cycles delay + 160 machine cycles
        if elapsed < 8 + 160 * 4 {
            return;
        }

        // Finish running
        self.ppu.dma_block_oam = false;
        self.ppu.dma_running = false;

        // Copy memory
        let start = if self.ppu.dma >= 0xFE {
            self.ppu.dma - 0x20
        } else {
            self.ppu.dma
        } as usize
            * 0x100;

        for (index, address) in (start..start + 160).enumerate() {
            self.ppu.oam[index] = match address {
                0x8000..=0x9FFF => self.ppu.vram[address as usize - 0x8000],
                _ => self.internal_read(address),
            };
        }
    }

    fn machine_cycle(&mut self) {
        match self.ppu.machine_cycle() {
            (true, true) => {
                self.update_screen();
                self.interrupt_handler.set(VBlank);
                self.interrupt_handler.set(Stat);
            }
            (true, false) => {
                self.update_screen();
                self.interrupt_handler.set(VBlank)
            }
            (false, true) => self.interrupt_handler.set(Stat),
            (false, false) => (),
        };

        if self.timer.machine_cycle() {
            self.interrupt_handler.set(Timing)
        };

        if self.serial.machine_cycle() {
            self.interrupt_handler.set(Serial)
        };

        if self
            .window
            .as_ref()
            .map(|window| self.joypad.machine_cycle(window))
            .unwrap_or(false)
        {
            self.interrupt_handler.set(Input)
        }
    }

    fn update_screen(&mut self) {
        if let Some(window) = self.window.as_mut() {
            window
                .update_with_buffer(self.ppu.screen.as_slice(), 160, 144)
                .unwrap()
        }
    }

    fn init_memory(mut mem: MemoryManagementUnit, rom: &[u8]) -> MemoryManagementUnit {
        rom.iter().enumerate().for_each(|(i, &v)| mem.memory[i] = v);

        if mem.boot_rom.is_some() {
            return mem;
        }

        macro_rules! set_memory {
            { $($addr:literal: $val:literal,)* } =>
            { $(mem.internal_write($addr, $val);)* }
        }

        set_memory! {
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
