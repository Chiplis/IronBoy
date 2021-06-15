use instruction::Instruction;

use crate::instruction_fetcher::{fetch_instruction, Gameboy};
use crate::register::{FlagRegister, ProgramCounter, RegisterId, ByteRegister};
use std::{thread, time, env};
use crate::memory_map::MemoryMap;
use crate::register::WordRegister::StackPointer;
use minifb::{Key, WindowOptions, Window};

mod instruction_fetcher;
mod instruction;
mod register;
mod instruction_executor;
mod memory_map;
mod ppu;
mod interrupt;

fn execute(gameboy: Gameboy, instruction: Instruction) -> Gameboy {
    match instruction {
        _ => panic!(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let rom = std::fs::read(args.get(1).unwrap()).unwrap();
    let mem = &mut MemoryMap::new(&rom);

    let mut gameboy = Gameboy {
        a: ByteRegister(0x01, RegisterId::A),
        b: ByteRegister(0x00, RegisterId::B),
        c: ByteRegister(0x13, RegisterId::C),
        d: ByteRegister(0x00, RegisterId::D),
        e: ByteRegister(0xD8, RegisterId::E),
        h: ByteRegister(0x01, RegisterId::H),
        l: ByteRegister(0x4D, RegisterId::L),
        f: FlagRegister { z: true, n: false, h: true, c: true },
        sp: StackPointer(0xFFFE),
        pc: ProgramCounter(0x0100),
        mem,
        vram: [0; 2 * 8 * 1024],
        ime_counter: -1,
        ime: false,
    };
    loop {
        let next_instruction = instruction_fetcher::fetch_instruction(&gameboy);
        let cycles = instruction_executor::execute_instruction(&mut gameboy, next_instruction);
        gameboy.mem.cycle(cycles);
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