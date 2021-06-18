use std::{env, thread, time};

use minifb::{Key, Window, WindowOptions};

use gameboy::Gameboy;
use instruction::Command;

use crate::instruction_fetcher::fetch_instruction;
use crate::memory_map::MemoryMap;
use crate::register::{ByteRegister, FlagRegister, ProgramCounter, RegisterId};
use crate::register::WordRegister::StackPointer;

mod instruction_fetcher;
mod instruction;
mod register;
mod instruction_executor;
mod memory_map;
mod ppu;
mod interrupt;
mod timer;
mod gameboy;

fn main() {
    let args: Vec<String> = env::args().collect();
    let rom = std::fs::read(args.get(1).unwrap()).unwrap();
    let mem = MemoryMap::new(&rom);

    let mut gameboy = Gameboy::new(mem);
    loop {
        let cycles = instruction_executor::execute_instruction(&mut gameboy);
        gameboy.mem.cycle(cycles as usize);
        //thread::sleep(time::Duration::from_millis(100));
    }
}

fn render() {
    let mut buffer: Vec<u32> = vec![0; 160 * 144];

    let mut window = Window::new(
        "Test - ESC to exit",
        160,
        144,
        WindowOptions::default(),
    )
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

    // Limit to max ~60 fps update rate
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = u32::from_le_bytes([0, 0, 255, 0]); // write something more funny here!
        }

        // We unwrap here as we want this code to exit if it fails. Real applications may want to handle this in a different way
        window
            .update_with_buffer(&buffer, 160, 144)
            .unwrap();
    }
}