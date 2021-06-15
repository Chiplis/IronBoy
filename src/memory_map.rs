use std::convert::TryInto;
use std::fmt::Display;
use std::ops::{Index, IndexMut, RangeInclusive, MulAssign};
use std::slice::Iter;

use crate::interrupt::{Interrupt, InterruptId};
use crate::interrupt::InterruptId::{Joypad, Serial, STAT, Timer, VBlank};
use crate::ppu::{PPU, PpuState, RenderResult};
use crate::ppu::PpuState::{OamSearch, PixelTransfer};
use crate::register::{ByteRegister, WordRegister};
use std::any::{Any, TypeId};

impl <Address: 'static + Into<u16>> Index<Address> for MemoryMap {
    type Output = u8;
    fn index(&self, address: Address) -> &Self::Output {
        if address.type_id() == TypeId::of::<u8>() {
            self.read(address.into() + 0xFF00)
        } else {
            self.read(address.into())
        }
    }
}

impl <Address: 'static + Into<u16> + Copy, Value: Into<u8> + Copy> MulAssign<(Address, Value)> for &mut MemoryMap {
    fn mul_assign(&mut self, (address, value): (Address, Value)) {
        if (address.into() == 53246 || address.into() == 53245) {
            print!("")
        }
        if address.type_id() == TypeId::of::<u8>() {
            self.write(address.into() + 0xFF00, value.into())
        } else {
            self.write(address.into(), value.into())
        }
    }
}

pub struct MemoryMap {
    memory: [u8; 0x10000],
    pub(crate) interrupt: Interrupt,
    ppu: PPU,
    invalid: u8,
    rom_size: usize,
}

impl MemoryMap {
    pub fn new(rom: &Vec<u8>) -> MemoryMap {
        let ppu = PPU::new();
        let interrupt = Interrupt::new();
        let mut mem = MemoryMap {
            ppu,
            interrupt,
            memory: [0; 0x10000],
            rom_size: rom.len() as usize,
            invalid: 0xFF,
        };
        MemoryMap::init_memory(&mut mem, rom);
        mem
    }

    fn read<T: Into<usize> + Display + Copy>(&self, address: T) -> &u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory(address.into()));
        match self.ppu.read(address.into()) {
            Some(value) => value,
            None => match self.interrupt.read(address.into()) {
                Some(value) => &value,
                None => &self.memory[address.into()]
            }
        }
    }

    fn write<T: Into<usize> + Copy>(&mut self, address: T, value: u8) {
        //println!("Writing address {}", address.into());
        if !(self.ppu.write(address.into(), value) || self.interrupt.write(address.into(), value)) {
            if address.into() >= self.rom_size {
                self.memory[address.into()] = value
            }
        }
    }

    pub fn cycle(&mut self, cpu_cycles: u8) {
        match self.ppu.render_cycle(cpu_cycles) {
            RenderResult::StateChange(_, PpuState::VBlank) => self.interrupt.set(VBlank, true),
            _ => {}
        }
    }

    pub(crate) fn init_memory(mut mem: &mut MemoryMap, rom: &Vec<u8>) {
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
    }
}