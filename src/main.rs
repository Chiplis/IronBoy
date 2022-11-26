extern crate core;

use std::thread;

use gameboy::Gameboy;

use crate::mmu::MemoryManagementUnit;
use std::time::{Duration, Instant};

use std::fs::{read, File, write, remove_file};
use std::io::{Read, Write};
use std::path::Path;

use crate::cartridge::Cartridge;
use crate::register::Register;

use clap::Parser;
use pixels::wgpu::PresentMode;
use pixels::{PixelsBuilder, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::VirtualKeyCode::{Back, Down, Escape, Left, Return, Right, Up, C, F, S, Z};
use winit::event_loop::EventLoop;
use winit::window::Fullscreen::Borderless;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

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
    let rom_path = args.rom_file;

    let mut gameboy = if rom_path.ends_with(".gb") || rom_path.ends_with(".gbc") {
        let rom = read(rom_path.clone()).expect("Unable to read ROM file");
        let cartridge = Cartridge::new(&rom);
        let mem = MemoryManagementUnit::new(rom, cartridge, args.boot_rom, Path::new(&rom_path));
        Gameboy::new(mem)
    } else {
        let save_file = &mut vec![];
        File::open(rom_path.clone())
            .unwrap()
            .read_to_end(save_file)
            .unwrap();
        serde_json::de::from_slice(save_file.as_slice()).unwrap()
    };

    if args.cold_boot {
        gameboy.reg = Register::new(gameboy.mmu.boot_rom.is_some())
    }

    let event_loop = EventLoop::new();

    let mut input = WinitInputHelper::new();
    let window = WindowBuilder::new()
        .with_title(rom_path.clone())
        .with_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_min_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_resizable(true)
        .with_visible(true)
        .with_fullscreen(Some(Borderless(None)))
        .build(&event_loop)
        .unwrap();
    let (width, height) = (WIDTH as u32, HEIGHT as u32);
    let pixels = PixelsBuilder::new(width, height, SurfaceTexture::new(width, height, &window))
        .present_mode(PresentMode::AutoNoVsync)
        .build()
        .unwrap();

    gameboy.mmu.renderer.set_pixels(pixels);

    gameboy.mmu.mbc.start();

    let mut frames: usize = 0;
    let start = Instant::now();
    let mut slowest_frame = Duration::from_nanos(0);

    event_loop.run(move |event, _target, control_flow| {
        let gameboy = &mut gameboy;

        frames += 1;
        let current_frame = run_frame(gameboy, sleep);
        if slowest_frame < current_frame {
            slowest_frame = current_frame
        }

        if !input.update(&event) {
            return;
        };

        if let Some(size) = input.window_resized() {
            if let Some(p) = gameboy.mmu.renderer.pixels().as_mut() { p.resize_surface(size.width, size.height) }
        }

        gameboy.mmu.joypad.held_action = [Z, C, Back, Return]
            .iter()
            .filter(|&&b| input.key_held(b)).copied()
            .collect();

        gameboy.mmu.joypad.held_direction = [Up, Down, Left, Right]
            .iter()
            .filter(|&&b| input.key_held(b)).copied()
            .collect();

        if frames % 600 == 0 {
            // Save temporary dummy file to prevent throttling on Apple Silicon
            write("feboy.tmp", vec![0; 0xFFFF]).unwrap();
        }

        if input.key_pressed(Escape) {
            if args.save_on_exit {
                save_state(rom_path.clone(), gameboy, ".esc.sav.json");
            }
            println!(
                "Finished running at {} FPS average, slowest frame took {:?}. Slowest render frame took {:?}.",
                frames as f64 / start.elapsed().as_secs_f64(),
                slowest_frame,
                gameboy.mmu.renderer.slowest
            );
            let tmp = Path::new("feboy.tmp");
            if tmp.exists() {
                remove_file(tmp).unwrap();
            }
            control_flow.set_exit();
        }

        if input.key_pressed(S) {
            save_state(rom_path.clone(), gameboy, ".sav.json");
        }

        if input.key_pressed(F) {
            sleep = !sleep;
            println!("Changed fast mode to {}", !sleep);
        }
    });
}

fn save_state(rom_path: String, gameboy: &mut Gameboy, append: &str) {
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

    if !sleep {
        return start.elapsed()
    }

    let expected = pin.1 + Duration::from_nanos(pin.0 * NANOS_PER_FRAME);
    if Instant::now() < expected {
        thread::sleep(expected - Instant::now());
        gameboy.pin = Some(pin);
    } else {
        gameboy.pin = None;
    }

    start.elapsed()
}
