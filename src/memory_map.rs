use crate::interrupt::{InterruptHandler};
use crate::interrupt::InterruptId::{JoypadInt, StatInt, TimerInt, VBlankInt};
use crate::ppu::{PPU, PpuMode, DmaState};
use std::any::{Any, TypeId};
use crate::ppu::RenderCycle::{StatTrigger, Normal};
use crate::ppu::PpuState::ModeChange;
use PpuMode::VBlank;
use crate::timer::{Timer};
use crate::joypad::{Joypad};
use DmaState::{Inactive, Starting};

pub struct MemoryMap {
    pub memory: Vec<u8>,
    pub interrupt_handler: InterruptHandler,
    pub ppu: PPU,
    timer: Timer,
    joypad: Joypad,
    rom_size: usize,
    rom_name: String,
    pub micro_ops: u16,
    dma_progress: usize,
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
        let micro_ops = 0;
        let dma_progress = 0;
        let mem = MemoryMap { joypad, ppu, interrupt_handler, timer, memory, rom_name, rom_size, micro_ops, dma_progress };
        MemoryMap::init_memory(mem, rom)
    }

    pub fn read<T: 'static + Into<usize> + Copy>(&mut self, address: T) -> u8 {
        let value = self.read_without_cycle(address);
        self.micro_cycle();
        return value;
    }

    pub(crate) fn read_without_cycle<T: 'static + Into<usize> + Copy>(&self, address: T) -> u8 {
        //println!("Reading address {} with value {}", address.into(), self.memory(address.into()));
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        let read = self.ppu.read(translated_address)
            .or(self.interrupt_handler.read(translated_address))
            .or(self.timer.read(translated_address))
            .or(self.joypad.read(translated_address))
            .unwrap_or(self.memory[translated_address]);
        read
    }

    fn write_mem<T: Into<usize> + Copy>(&mut self, address: T, value: u8, trigger_cycle: bool) {
        //println!("Writing address {}", address.into());
        let address = address.into();
        if !(self.ppu.write(address, value)
            || self.timer.write(address, value)
            || self.interrupt_handler.write(address, value)
            || self.joypad.write(address, value)) && (address >= self.rom_size) {
            self.memory[address] = value
        }
        if trigger_cycle { self.micro_cycle() }
    }

    pub fn write<Address: 'static + Into<usize> + Copy, Value: Into<u8> + Copy>(&mut self, address: Address, value: Value) {
        let translated_address = if address.type_id() == TypeId::of::<u8>() { address.into() + 0xFF00 } else { address.into() };
        self.write_mem(translated_address, value.into(), true)
    }

    pub fn micro_cycle(&mut self) {
        self.micro_ops += 1;
        self.dma_transfer();
        self.cycle(1);
    }

    fn dma_transfer(&mut self) {
        if let Inactive | Starting = self.ppu.dma { return; }
        while self.dma_progress < self.ppu.dma_progress {
            self.ppu.oam[self.dma_progress] = self.read_without_cycle(self.ppu.dma_offset * 0x100 + self.dma_progress);
            self.dma_progress += 1;
        }
        if self.dma_progress == self.ppu.oam.len() {
            self.dma_progress = 0;
        }
    }

    pub fn cycle(&mut self, cpu_cycles: usize) {
        let mut interrupts = vec![];
        interrupts.append(&mut match self.ppu.render_cycle(cpu_cycles) {
            StatTrigger(ModeChange(_, VBlank)) => vec![VBlankInt, StatInt],
            Normal(ModeChange(_, VBlank)) => vec![VBlankInt],
            StatTrigger(_) => vec![StatInt],
            _ => vec![]
        });
        interrupts.append(&mut match self.timer.timer_cycle(cpu_cycles as u16) {
            Some(_) => vec![TimerInt],
            None => vec![],
        });

        interrupts.append(&mut self.joypad.input_cycle(&self.ppu.window).iter().map(|_| JoypadInt).collect());

        self.interrupt_handler.set(interrupts, true);
    }

    fn init_memory(mut mem: MemoryMap, rom: &Vec<u8>) -> MemoryMap {
        for (index, value) in rom.iter().enumerate() { mem.memory[index] = *value }
        mem.write_mem(0xFF05_u16, 0, false);
        mem.write_mem(0xFF06_u16, 0, false);
        mem.write_mem(0xFF07_u16, 0, false);
        mem.write_mem(0xFF10_u16, 0x80, false);
        mem.write_mem(0xFF11_u16, 0xBF, false);
        mem.write_mem(0xFF12_u16, 0xF3, false);
        mem.write_mem(0xFF14_u16, 0xBF, false);
        mem.write_mem(0xFF16_u16, 0x3F, false);
        mem.write_mem(0xFF16_u16, 0x3F, false);
        mem.write_mem(0xFF17_u16, 0, false);
        mem.write_mem(0xFF19_u16, 0xBF, false);
        mem.write_mem(0xFF1A_u16, 0x7F, false);
        mem.write_mem(0xFF1B_u16, 0xFF, false);
        mem.write_mem(0xFF1C_u16, 0x9F, false);
        mem.write_mem(0xFF1E_u16, 0xFF, false);
        mem.write_mem(0xFF20_u16, 0xFF, false);
        mem.write_mem(0xFF21_u16, 0, false);
        mem.write_mem(0xFF22_u16, 0, false);
        mem.write_mem(0xFF23_u16, 0xBF, false);
        mem.write_mem(0xFF24_u16, 0x77, false);
        mem.write_mem(0xFF25_u16, 0xF3, false);
        mem.write_mem(0xFF26_u16, 0xF1, false);
        mem.write_mem(0xFF40_u16, 0x91, false);
        mem.write_mem(0xFF42_u16, 0, false);
        mem.write_mem(0xFF43_u16, 0, false);
        mem.write_mem(0xFF45_u16, 0, false);
        mem.write_mem(0xFF47_u16, 0xFC, false);
        mem.write_mem(0xFF48_u16, 0xFF, false);
        mem.write_mem(0xFF49_u16, 0xFF, false);
        mem.write_mem(0xFF4A_u16, 0, false);
        mem.write_mem(0xFF4B_u16, 0, false);
        mem.write_mem(0xFF00_u16, 0xFF, false);
        mem
    }
}