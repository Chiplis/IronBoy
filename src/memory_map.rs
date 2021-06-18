use std::convert::TryInto;
use std::fmt::Display;
use std::ops::{Index, IndexMut, RangeInclusive, MulAssign};
use std::slice::Iter;

use crate::interrupt::{InterruptHandler, InterruptId};
use crate::interrupt::InterruptId::{JoypadInt, SerialInt, StatInt, TimerInt, VBlankInt};
use crate::ppu::{PPU, PpuMode, PpuState};
use crate::ppu::PpuMode::{OamSearch, PixelTransfer};
use crate::register::{ByteRegister, WordRegister};
use std::any::{Any, TypeId};
use crate::ppu::RenderCycle::{StatTrigger, Normal};
use crate::ppu::PpuState::ModeChange;
use PpuMode::VBlank;
use crate::timer::{Timer, TimerInterrupt};

impl <Address: 'static + Into<u16>> Index<Address> for MemoryMap {
    type Output = u8;
    fn index(&self, address: Address) -> &Self::Output {
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.read(translated_address)
    }
}

impl <Address: 'static + Into<u16> + Copy, Value: Into<u8> + Copy> MulAssign<(Address, Value)> for MemoryMap {
    fn mul_assign(&mut self, (address, value): (Address, Value)) {
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.write(translated_address, value.into());
    }
}

pub struct MemoryMap {
    memory: [u8; 0x10000],
    pub interrupt_handler: InterruptHandler,
    pub ppu: PPU,
    pub timer: Timer,
    invalid: u8,
    rom_size: usize,
}

impl MemoryMap {
    pub fn new(rom: &Vec<u8>) -> MemoryMap {
        let ppu = PPU::new();
        let interrupt = InterruptHandler::new();
        let timer = Timer::new();
        let mut mem = MemoryMap {
            ppu,
            interrupt_handler: interrupt,
            timer,
            memory: [0; 0x10000],
            rom_size: rom.len() as usize,
            invalid: 0xFF,
        };
        MemoryMap::init_memory(mem, rom)
    }

    fn read<T: 'static + Into<usize> + Display + Copy>(&self, address: T) -> &u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory(address.into()));
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.ppu.read(translated_address)
            .or(self.interrupt_handler.read(translated_address))
            .or(self.timer.read(translated_address))
            .unwrap_or(&self.memory[translated_address])
    }

    fn write<T: Into<usize> + Copy>(&mut self, address: T, value: u8) {
        //println!("Writing address {}", address.into());
        if !(self.ppu.write(address.into(), value)
            || self.timer.write(address.into(), value)
            || self.interrupt_handler.write(address.into(), value)) {
            if address.into() >= self.rom_size { self.memory[address.into()] = value }
        }
    }

    pub fn cycle(&mut self, cpu_cycles: usize) {
        match self.ppu.render_cycle(cpu_cycles) {
            StatTrigger(ModeChange(_, VBlank)) => { self.interrupt_handler.set(vec![VBlankInt, StatInt], true) },
            Normal(ModeChange(_, VBlank)) => { self.interrupt_handler.set(vec![VBlankInt], true) }
            _ => {}
        };
        match self.timer.timer_cycle(cpu_cycles) {
            Some(TimerInterrupt()) => { self.interrupt_handler.set(vec![TimerInt], true) },
            None => {}
        };
    }

    fn init_memory(mut mem: MemoryMap, rom: &Vec<u8>) -> MemoryMap {
        for (index, value) in rom.iter().enumerate() {
            mem.memory[index] = *value
        }
        mem *= (0xFF05 as u16, 0);
        mem *= (0xFF06 as u16, 0);
        mem *= (0xFF07 as u16, 0);
        mem *= (0xFF10 as u16, 0x80);
        mem *= (0xFF11 as u16, 0xBF);
        mem *= (0xFF12 as u16, 0xF3);
        mem *= (0xFF14 as u16, 0xBF);
        mem *= (0xFF16 as u16, 0x3F);
        mem *= (0xFF16 as u16, 0x3F);
        mem *= (0xFF17 as u16, 0);
        mem *= (0xFF19 as u16, 0xBF);
        mem *= (0xFF1A as u16, 0x7F);
        mem *= (0xFF1B as u16, 0xFF);
        mem *= (0xFF1C as u16, 0x9F);
        mem *= (0xFF1E as u16, 0xFF);
        mem *= (0xFF20 as u16, 0xFF);
        mem *= (0xFF21 as u16, 0);
        mem *= (0xFF22 as u16, 0);
        mem *= (0xFF23 as u16, 0xBF);
        mem *= (0xFF24 as u16, 0x77);
        mem *= (0xFF25 as u16, 0xF3);
        mem *= (0xFF26 as u16, 0xF1);
        mem *= (0xFF40 as u16, 0x91);
        mem *= (0xFF42 as u16, 0);
        mem *= (0xFF43 as u16, 0);
        mem *= (0xFF45 as u16, 0);
        mem *= (0xFF47 as u16, 0xFC);
        mem *= (0xFF48 as u16, 0xFF);
        mem *= (0xFF49 as u16, 0xFF);
        mem *= (0xFF4A as u16, 0);
        mem *= (0xFF4B as u16, 0);
        mem *= (0xFF00 as u16, 0xFF);
        mem
    }
}