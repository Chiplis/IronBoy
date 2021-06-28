use std::ops::{MulAssign};
use crate::interrupt::{InterruptHandler};
use crate::interrupt::InterruptId::{JoypadInt, StatInt, TimerInt, VBlankInt};
use crate::ppu::{PPU, PpuMode};
use std::any::{Any, TypeId};
use crate::ppu::RenderCycle::{StatTrigger, Normal};
use crate::ppu::PpuState::ModeChange;
use PpuMode::VBlank;
use crate::timer::{Timer};
use crate::joypad::{Joypad};

impl<Address: 'static + Into<usize> + Copy, Value: Into<u8> + Copy> MulAssign<(Address, Value)> for MemoryMap {
    fn mul_assign(&mut self, (address, value): (Address, Value)) {
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.write(translated_address, value.into());
    }
}

pub struct MemoryMap {
    pub memory: Vec<u8>,
    pub interrupt_handler: InterruptHandler,
    pub ppu: PPU,
    timer: Timer,
    joypad: Joypad,
    rom_size: usize,
    rom_name: String,
    pub(crate) reads: u16,
    pub writes: u16,
}

impl MemoryMap {
    pub fn new(rom: &Vec<u8>, rom_name: &String) -> MemoryMap {
        let ppu = PPU::new(rom_name);
        let joypad = Joypad::new();
        let interrupt_handler = InterruptHandler::new();
        let timer = Timer::new();
        let rom_size = rom.len() as usize;
        let rom_name = rom_name.to_owned();
        let memory = vec![0; 0x10000];
        let (reads, writes) = (0, 0);
        let mem = MemoryMap { joypad, ppu, interrupt_handler, timer, memory, rom_name, rom_size, reads, writes };
        MemoryMap::init_memory(mem, rom)
    }

    pub fn read<T: 'static + Into<usize> + Copy>(&mut self, address: T) -> u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory(address.into()));
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.reads += 1;
        let ret = self.ppu.read(translated_address)
            .or(self.interrupt_handler.read(translated_address))
            .or(self.timer.read(translated_address))
            .or(self.joypad.read(translated_address))
            .unwrap_or(self.memory[translated_address]);
        self.cycle(1);
        ret
    }

    pub fn write<T: Into<usize> + Copy>(&mut self, address: T, value: u8) {
        //println!("Writing address {}", address.into());
        let address = address.into();
        self.writes += 1;
        if !(self.ppu.write(&self.memory, address, value)
            || self.timer.write(address, value)
            || self.interrupt_handler.write(address, value)
            || self.joypad.write(address, value)) {
            if address >= self.rom_size || self.rom_name.contains("cpu_instrs.gb") {
                self.memory[address] = value
            }
        }
        self.cycle(1);
    }

    pub fn cycle(&mut self, cpu_cycles: usize) {
        let mut interrupts = vec![];
        interrupts.append(&mut match self.ppu.render_cycle(cpu_cycles) {
            StatTrigger(ModeChange(_, VBlank)) => vec![VBlankInt, StatInt],
            Normal(ModeChange(_, VBlank)) => vec![VBlankInt],
            StatTrigger(_) => vec![StatInt],
            _ => vec![]
        });
        interrupts.append(&mut match self.timer.timer_cycle(cpu_cycles) {
            Some(_) => vec![TimerInt],
            None => vec![],
        });

        interrupts.append(&mut self.joypad.input_cycle(&self.ppu.window).iter().map(|_| JoypadInt).collect());

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
        mem.writes = 0;
        mem.reads = 0;
        mem
    }
}