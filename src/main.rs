use std::{env, thread, io};

use gameboy::Gameboy;

use crate::memory_map::MemoryMap;
use std::time::{Instant, Duration};
use std::process::Command;
use std::path::Path;

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
    let cycle_duration = 1.0_f64 / FREQUENCY as f64;
    let mut start = Instant::now();
    loop {
        while elapsed_cycles < FREQUENCY / 60 {
            let halted = gameboy.halted;
            let cycles = gameboy.cycle() as u16;
            elapsed_cycles += cycles as u32 * 4;
            let mem_cycles = cycles - gameboy.mem.micro_ops;
            if mem_cycles != 0 && !halted && !gameboy.halted {
                panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mem.micro_ops)
            } else if mem_cycles != 0 {
                for _ in 0..mem_cycles {
                    gameboy.mem.micro_cycle();
                }
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

#[test]
fn test_roms() -> Result<(), io::Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    for entry in std::fs::read_dir(std::env::var("HOME").unwrap() + &String::from("/femboy/tests"))? {
        let entry = entry?;
        let path = entry.path();
        let rom = String::from(path.to_str().unwrap());
        if !rom.ends_with(".gb") { continue; }

        let rom_vec = std::fs::read(rom.clone()).unwrap();
        if rom_vec.len() > 32768 {
            println!("Still need to implement MBC for larger ROM's: {}", rom.clone());
            continue;
        }
        let mem = MemoryMap::new(&rom_vec, &rom);
        let mut gameboy = Gameboy::new(mem);
        let mut elapsed_cycles = 0;
        let cycle_duration = 1.0_f64 / FREQUENCY as f64;
        let mut start = Instant::now();

        //println!("{}", rom);
        let mut spawn = true;
        'inner: loop {
            if spawn {
                let rom_clone = rom.clone();
                let tx_clone = tx.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(8));
                    Command::new(format!("wmctrl")).args(&["-a", rom_clone.as_str()]).status().unwrap_or_else(|_| std::process::exit(1));
                    let process_id = {
                        let xdotool = Command::new("xdotool").args(&["getwindowfocus", "-f"]).output().unwrap_or_else(|_| std::process::exit(1));
                        String::from_utf8(xdotool.stdout).unwrap().as_str().to_owned()
                    };
                    let screenshot_path = rom_clone.as_str().split("/").collect::<Vec<&str>>();
                    let rom_name = screenshot_path.last().unwrap();
                    let screenshot_path = screenshot_path[0..screenshot_path.len() - 2].join("/");
                    let screenshot = Command::new("import")
                        .args(&["-window", process_id.as_str(), &(screenshot_path.to_owned() + "/test_output/" + rom_name + ".png")])
                        .output()
                        .unwrap_or_else(|_| std::process::exit(1));
                    tx_clone.send("");
                });
                spawn = false;
            }

            for _ in rx.try_recv() { break 'inner }

            while elapsed_cycles < FREQUENCY / 60 {
                let halted = gameboy.halted;
                let cycles = gameboy.cycle() as u16;
                elapsed_cycles += cycles as u32 * 4;
                let mem_cycles = cycles - gameboy.mem.micro_ops;
                if mem_cycles != 0 && !halted && !gameboy.halted {
                    panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mem.micro_ops)
                } else if mem_cycles != 0 {
                    for _ in 0..mem_cycles {
                        gameboy.mem.micro_cycle();
                    }
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
    Ok(())
}