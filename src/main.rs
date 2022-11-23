extern crate core;

use std::thread;

use gameboy::Gameboy;

use crate::mmu::MemoryManagementUnit;
use std::time::{Duration, Instant};

use minifb::Key::{Escape, F, S};
use std::fs::{read, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::cartridge::Cartridge;
use crate::register::Register;

use clap::Parser;
use minifb::{KeyRepeat, Scale, ScaleMode, Window, WindowOptions};
use KeyRepeat::No;

mod cartridge;
mod gameboy;
mod instruction;
mod instruction_fetcher;
mod interrupt;
mod joypad;
mod mbc;
mod mbc0;
mod mbc1;
mod mbc3;
mod mmu;
mod ppu;
mod register;
mod renderer;
mod serial;
mod timer;

#[cfg(test)]
mod test;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// GameBoy ROM file to input
    rom_file: String,

    /// Runs the emulator without a backing window, used during test execution
    #[clap(long, default_value = "false")]
    headless: bool,

    /// Boot title screen even when opening save file
    #[clap(long, default_value = "false")]
    cold_boot: bool,

    /// Wait between frames to attempt to lock framerate to 60 FPS
    #[clap(long, default_value = "false")]
    fast: bool,

    /// Automatically save state before exiting emulator
    #[clap(long, default_value = "false")]
    save_on_exit: bool,

    /// Use specified boot ROM
    #[clap(long)]
    boot_rom: Option<String>,
}

fn main() {
    let args = Args::parse();
    let mut sleep = !args.fast;
    let rom_path = Path::new(&args.rom_file);
    if !rom_path.exists() {
        panic!("The input ROM path doesn't exist")
    }
    if !rom_path.is_file() {
        panic!("The input ROM isn't a file")
    }

    let rom_path = rom_path.to_str().unwrap();

    let mut gameboy = if rom_path.ends_with(".gb") || rom_path.ends_with(".gbc") {
        let rom = read(rom_path).expect("Unable to read ROM file");
        let cartridge = Cartridge::new(&rom);
        let mem = MemoryManagementUnit::new(rom, cartridge, args.boot_rom, Path::new(&args.rom_file));
        Gameboy::new(mem)
    } else {
        let save_file = &mut vec![];
        File::open(rom_path)
            .unwrap()
            .read_to_end(save_file)
            .unwrap();
        serde_json::de::from_slice(save_file.as_slice()).unwrap()
    };

    if args.cold_boot {
        gameboy.reg = Register::new(gameboy.mmu.boot_rom.is_some())
    }

    if !args.headless {
        gameboy.mmu.renderer.set_window(
            Window::new(
                &args.rom_file,
                WIDTH,
                HEIGHT,
                WindowOptions {
                    borderless: false,
                    transparency: false,
                    title: true,
                    resize: true,
                    scale: Scale::X1,
                    scale_mode: ScaleMode::Stretch,
                    topmost: false,
                    none: false,
                },
            )
            .unwrap(),
        )
    }

    gameboy.mmu.mbc.start();

    let mut frames: usize = 0;
    let start = Instant::now();
    let mut slowest_frame = Duration::from_nanos(0);
    loop {
        frames += 1;
        let current_frame = run_frame(&mut gameboy, sleep);
        if slowest_frame < current_frame {
            slowest_frame = current_frame
        }

        let window = gameboy.mmu.renderer.window().as_ref();

        if window.map_or(false, |w| w.is_key_pressed(Escape, No)) {
            if args.save_on_exit {
                save_state(rom_path, &mut gameboy, ".esc.sav.json");
            }
            break;
        } else if window.map_or(false, |w| w.is_key_pressed(S, No)) {
            save_state(rom_path, &mut gameboy, ".sav.json");
        } else if window.map_or(false, |w| w.is_key_pressed(F, No)) {
            sleep = !sleep;
            println!("Changed fast mode to {}", !sleep);
        }
    }
    println!(
        "Finished running at {} FPS average, slowest frame took {:?} seconds to render",
        frames as f64 / start.elapsed().as_secs_f64(),
        slowest_frame
    );
}

fn save_state(rom_path: &str, gameboy: &mut Gameboy, append: &str) {
    println!("Saving state...");
    let append = if !rom_path.ends_with(append) {
        append
    } else {
        ""
    };

    gameboy.mmu.mbc.save();

    let mut save_file = File::create(rom_path.to_string() + append).unwrap();
    save_file
        .write_all(serde_json::ser::to_vec(gameboy).unwrap().as_slice())
        .unwrap();
    println!("Savefile {}{} successfully generated.", rom_path, append);
}

const CYCLES_PER_FRAME: u16 = 17556;
const NANOS_PER_FRAME: u64 = 16742706;

fn run_frame(gameboy: &mut Gameboy, sleep: bool) -> Duration {
    let mut elapsed_cycles = 0;
    let start = Instant::now();
    let pin = if let Some(pin) = gameboy.pin {
        (pin.0 + 1, pin.1)
    } else {
        (1, Instant::now())
    };
    while elapsed_cycles < CYCLES_PER_FRAME {
        let previously_halted = gameboy.halted;
        let cycles = gameboy.cycle() as u16;
        elapsed_cycles += cycles;
        let mem_cycles = cycles - gameboy.mmu.cycles;
        if mem_cycles != 0 && !previously_halted && !gameboy.halted {
            panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mmu.cycles)
        } else if mem_cycles == 1 {
            gameboy.mmu.cycle(4)
        } else {
            for _ in 0..mem_cycles {
                gameboy.mmu.cycle(4)
            }
        }
        gameboy.mmu.cycles = 0;
    }
    if sleep {
        let expected = pin.1 + Duration::from_nanos(pin.0 * NANOS_PER_FRAME);
        if Instant::now() < expected {
            thread::sleep(expected - Instant::now());
            gameboy.pin = Some(pin);
        } else {
            gameboy.pin = None;
        }
    }
    start.elapsed()
}
