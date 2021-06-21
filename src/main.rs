use std::{env, thread, time};

use minifb::{Key, Window, WindowOptions};

use gameboy::Gameboy;
use instruction::Command;

use crate::memory_map::MemoryMap;
use crate::register::{ByteRegister, FlagRegister, ProgramCounter, RegisterId};
use crate::register::WordRegister::StackPointer;

mod instruction_fetcher;
mod instruction;
mod register;
mod memory_map;
mod ppu;
mod interrupt;
mod timer;
mod gameboy;
mod input;

fn main() {
    let args: Vec<String> = env::args().collect();
    let rom_name = args.get(1).unwrap();
    let rom = std::fs::read(rom_name).unwrap();
    let mem = MemoryMap::new(&rom, rom_name);

    let mut gameboy = Gameboy::new(mem);
    loop {
        let cycles = gameboy.cycle();
        gameboy.mem.cycle(cycles as usize);
        //thread::sleep(time::Duration::from_millis(100));
    }
}

fn render() {
    let mut buffer: Vec<u32> = vec![0; 160 * 144];


}