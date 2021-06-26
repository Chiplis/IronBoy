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
use crate::input::{InputReceiver, InputSender};
use minifb::InputCallback;
use std::cell::{RefCell, RefMut};


impl<Address: 'static + Into<usize> + Copy, Value: Into<u8> + Copy> MulAssign<(Address, Value)> for MemoryMap {
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
    pub input_receiver: InputReceiver,
    rom_size: usize,
    rom_name: String,
}

impl MemoryMap {
    pub fn new(rom: &Vec<u8>, rom_name: &String) -> MemoryMap {
        let mut ppu = PPU::new();
        let (input_send, input_recv) = std::sync::mpsc::channel();
        let input_sender = Box::new(InputSender::new(input_send));
        let input_receiver = InputReceiver::new(input_recv);
        ppu.window.set_input_callback(input_sender);
        let interrupt_handler = InterruptHandler::new();
        let timer = Timer::new();
        let rom_size = rom.len() as usize;
        let rom_name = rom_name.to_owned();
        let memory = [0; 0x10000];
        let mut mem = MemoryMap { input_receiver, ppu, interrupt_handler, timer, memory, rom_name, rom_size };
        MemoryMap::init_memory(mem, rom)
    }

    pub(crate) fn read<T: 'static + Into<usize> + Copy>(&self, address: T) -> u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory(address.into()));
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.ppu.read(translated_address)
            .or(self.interrupt_handler.read(translated_address))
            .or(self.timer.read(translated_address))
            .unwrap_or(self.memory[translated_address])
    }

    pub(crate) fn write<T: Into<usize> + Copy>(&mut self, address: T, value: u8) {
        //println!("Writing address {}", address.into());
        let address = address.into();
        if !(self.ppu.write(self.memory, address, value)
            || self.timer.write(address, value)
            || self.interrupt_handler.write(address, value)) {
            if address >= self.rom_size || self.rom_name.contains("cpu_instrs.gb") {
                self.memory[address] = value
            }
        }
    }

    pub fn cycle(&mut self, cpu_cycles: usize) {
        let mut interrupts = vec![];
        interrupts.append(&mut match self.ppu.render_cycle(cpu_cycles) {
            StatTrigger(ModeChange(_, VBlank)) => vec![VBlankInt, StatInt],
            Normal(ModeChange(_, VBlank)) => vec![VBlankInt],
            _ => vec![]
        });
        interrupts.append(&mut match self.timer.timer_cycle(cpu_cycles) {
            Some(_) => vec![TimerInt],
            None => vec![],
        });
        interrupts.append(&mut match self.input_receiver.input_cycle() {
            Some(_) => vec![JoypadInt],
            _ => vec![]
        });
        self.interrupt_handler.set(interrupts, true);
    }

    fn init_memory(mut mem: MemoryMap, rom: &Vec<u8>) -> MemoryMap {
        for (index, value) in rom.iter().enumerate() { mem.memory[index] = *value }
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