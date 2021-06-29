use std::{env, thread};

use gameboy::Gameboy;

use crate::memory_map::MemoryMap;
use std::time::{Instant, Duration};

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
    let cycle_duration = 1.0 as f64 / FREQUENCY as f64;
    let mut start = Instant::now();
    loop {
        while elapsed_cycles < FREQUENCY / 60 {
            let cycles = gameboy.cycle() as u16;
            elapsed_cycles += cycles as u32 * 4;
            let mem_cycles = cycles as i32 - gameboy.mem.micro_ops as i32;
            if mem_cycles != 0 {
                panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mem.micro_ops)
            }
            gameboy.mem.micro_ops = 0;
        }
        let cycles_time: f64 = cycle_duration * elapsed_cycles as f64;
        let sleep_time = cycles_time - start.elapsed().as_secs_f64();
        if sleep_time > 0.0 { thread::sleep(Duration::from_secs_f64(sleep_time)); }
        start = Instant::now();
        elapsed_cycles = 0;
    }
}