use std::thread;

use gameboy::Gameboy;

use crate::memory_map::MemoryMap;
use std::time::{Duration, Instant};

use minifb::Key::Escape;
use std::fs::read;
use std::path::Path;

use clap::Parser;

mod gameboy;
mod instruction;
mod instruction_fetcher;
mod interrupt;
mod joypad;
mod memory_map;
mod ppu;
mod register;
mod serial;
mod timer;
#[cfg(test)]
mod test;

const FREQUENCY: u32 = 4194304;
const WIDTH: usize = 160;
const HEIGHT: usize = 144;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// GameBoy ROM file to input
    rom_file: String,

    /// Toggle headless mode
    #[clap(long, default_value = "false")]
    headless: bool,

    /// Toggle waiting between frames
    #[clap(long, default_value = "false")]
    fast: bool,

    /// Sleep threshold between frames
    #[clap(long, default_value_t = 0.0)]
    threshold: f64,

    /// Use specified boot ROM
    #[clap(long)]
    boot_rom: Option<String>,
}

fn main() {
    let args = Args::parse();
    let (sleep, threshold) = (!args.fast, args.threshold);
    let rom_path = Path::new(&args.rom_file);
    if !rom_path.exists() {
        panic!("The input ROM path doesn't exist")
    }
    if !rom_path.is_file() {
        panic!("The input ROM isn't a file")
    }
    let rom = read(rom_path).expect("Unable to read ROM file");
    let mem = MemoryMap::new(&rom, rom_path.to_str().unwrap(), args.headless, args.boot_rom);

    let mut gameboy = Gameboy::new(mem);
    let mut frames: usize = 0;
    let start = Instant::now();
    let mut slowest_frame = 0.0;
    loop {
        frames += 1;
        let current_frame = run_frame(&mut gameboy, sleep, threshold);
        if slowest_frame < current_frame {
            slowest_frame = current_frame
        }
        if gameboy
            .mem
            .window
            .as_ref()
            .map(|window| window.is_key_down(Escape))
            .unwrap_or(false)
        {
            break;
        }
    }
    println!(
        "Finished running at {} FPS average, slowest frame took {} seconds to render",
        frames as f64 / start.elapsed().as_secs_f64(),
        slowest_frame
    );
}

fn run_frame(gameboy: &mut Gameboy, sleep: bool, threshold: f64) -> f64 {
    let mut elapsed_cycles = 0;
    const CYCLE_DURATION: f64 = 1.0_f64 / FREQUENCY as f64;
    let start = Instant::now();
    while elapsed_cycles < FREQUENCY / 60 {
        let previously_halted = gameboy.halted;
        let cycles = gameboy.cycle() as u16;
        elapsed_cycles += cycles as u32 * 4;
        let mem_cycles = cycles - gameboy.mem.cycles;
        if mem_cycles != 0 && !previously_halted && !gameboy.halted {
            panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mem.cycles)
        } else if mem_cycles == 1 {
            gameboy.mem.cycle()
        } else {
            for _ in 0..mem_cycles {
                gameboy.mem.cycle()
            }
        }
        gameboy.mem.cycles = 0;
    }
    if sleep {
        let cycles_time: f64 = CYCLE_DURATION * elapsed_cycles as f64;
        let sleep_time = cycles_time - start.elapsed().as_secs_f64();
        if sleep_time > threshold {
            thread::sleep(Duration::from_secs_f64(sleep_time));
        }
    }
    start.elapsed().as_secs_f64()
}
