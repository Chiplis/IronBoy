use std::{env, thread, time};

use minifb::{Key, Window, WindowOptions};

use gameboy::Gameboy;
use instruction::Command;

use crate::memory_map::MemoryMap;
use crate::register::{ByteRegister, FlagRegister, ProgramCounter, RegisterId};
use crate::register::WordRegister::StackPointer;
use std::time::{Instant, Duration};
use std::cmp::max;

mod instruction_fetcher;
mod instruction;
mod register;
mod memory_map;
mod ppu;
mod interrupt;
mod timer;
mod gameboy;
mod joypad;

const FREQUENCY: u32 = 4194304;

fn main() {
    let args: Vec<String> = env::args().collect();
    let rom_name = args.get(1).unwrap();
    let rom = std::fs::read(rom_name).unwrap();
    let mem = MemoryMap::new(&rom, rom_name);

    let mut gameboy = Gameboy::new(mem);
    let mut elapsed_cycles = 0;
    let cycle_duration = (1.0 as f64 / FREQUENCY as f64);
    let mut start = Instant::now();
    loop {
        while elapsed_cycles < FREQUENCY / 60 {
            let cycles = gameboy.cycle() as u32;
            elapsed_cycles += cycles * 4;
            gameboy.mem.cycle(cycles as usize);
        }
        let cycles_time: f64 = (cycle_duration * elapsed_cycles as f64);
        let sleep_time = cycles_time - start.elapsed().as_secs_f64();
        if sleep_time > 0.0 { thread::sleep(Duration::from_secs_f64(sleep_time)); }
        start = Instant::now();
        elapsed_cycles = 0;
    }
}

fn render() {
    let mut buffer: Vec<u32> = vec![0; 160 * 144];
}