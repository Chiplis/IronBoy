use instruction::Instruction;

use crate::instruction_fetcher::{fetch_instruction, Gameboy};
use crate::register::{FlagRegister, ProgramCounter, RegisterId, ByteRegister};
use std::{thread, time, env};
use crate::memory_map::MemoryMap;
use crate::register::SpecialRegister::StackPointer;

mod instruction_fetcher;
mod instruction;
mod register;
mod instruction_executor;
mod memory_map;

fn execute(gameboy: Gameboy, instruction: Instruction) -> Gameboy {
    match instruction {
        _ => panic!(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let rom = args.get(1).unwrap();
    let mut gameboy = Gameboy {
        a: ByteRegister(0x01, RegisterId::A),
        b: ByteRegister(0x00, RegisterId::B),
        c: ByteRegister(0x13, RegisterId::C),
        d: ByteRegister(0x00, RegisterId::D),
        e: ByteRegister(0xD8, RegisterId::E),
        h: ByteRegister(0x01, RegisterId::H),
        l: ByteRegister(0x4D, RegisterId::L),
        f: FlagRegister{z: true, n: false, h: true, c: true},
        sp: StackPointer(0xFFFE),
        pc: ProgramCounter(0x0100),
        memory: MemoryMap::new(),
        vram: [0; 2 * 8 * 1024],
        rom: std::fs::read(rom).unwrap(),
    };
    loop {
        let next_instruction = instruction_fetcher::fetch_instruction(&gameboy);
        println!(" | {:?}", next_instruction);
        instruction_executor::execute_instruction(&mut gameboy, next_instruction);
        //thread::sleep(time::Duration::from_millis(100));
    }
}