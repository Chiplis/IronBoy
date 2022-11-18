extern crate core;

use std::thread;

use gameboy::Gameboy;

use crate::mmu::MemoryManagementUnit;
use std::time::{Duration, Instant};

use minifb::Key::{Escape, S};
use std::fs::{read, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::cartridge::Cartridge;
use clap::Parser;

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

const FREQUENCY: u32 = 4194304;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// GameBoy ROM file to input
    rom_file: String,

    /// Runs the emulator without a backing window, used during test execution
    #[clap(long, default_value = "false")]
    headless: bool,

    /// Wait between frames to attempt to lock framerate to 60 FPS
    #[clap(long, default_value = "false")]
    fast: bool,

    /// Automatically save state before exiting emulator
    #[clap(long, default_value = "false")]
    save_on_exit: bool,

    /// Minimum threshold to trigger sleep between frames
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

    let rom_path = rom_path.to_str().unwrap();

    let mut gameboy = if rom_path.ends_with(".gb") || rom_path.ends_with(".gbc") {
        let rom = read(rom_path).expect("Unable to read ROM file");
        let cartridge = Cartridge::new(&rom);
        let mem = MemoryManagementUnit::new(rom, cartridge, args.boot_rom, &args.rom_file);
        Gameboy::new(mem)
    } else {
        let save_file = &mut vec![];
        File::open(rom_path)
            .unwrap()
            .read_to_end(save_file)
            .unwrap();
        serde_json::de::from_slice(save_file.as_slice()).unwrap()
    };

    let mut frames: usize = 0;
    let start = Instant::now();
    let mut slowest_frame = 0.0;
    loop {
        frames += 1;
        let current_frame = run_frame(&mut gameboy, sleep, threshold);
        if slowest_frame < current_frame {
            slowest_frame = current_frame
        }
        if renderer::instance()
            .as_ref()
            .map(|window| window.is_key_down(Escape))
            .unwrap_or(false)
        {
            if args.save_on_exit {
                save_state(rom_path, &mut gameboy, ".esc.sav.json");
            }
            break;
        } else if renderer::instance()
            .as_ref()
            .map(|window| window.is_key_down(S))
            .unwrap_or(false)
        {
            save_state(rom_path, &mut gameboy, ".sav.json");
        }
    }
    println!(
        "Finished running at {} FPS average, slowest frame took {} seconds to render",
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

fn run_frame(gameboy: &mut Gameboy, sleep: bool, threshold: f64) -> f64 {
    let mut elapsed_cycles = 0;
    const CYCLE_DURATION: f64 = 1.0_f64 / FREQUENCY as f64;
    let start = Instant::now();
    while elapsed_cycles < FREQUENCY / 60 {
        let previously_halted = gameboy.halted;
        let cycles = gameboy.cycle() as u16;
        elapsed_cycles += cycles as u32 * 4;
        let mem_cycles = cycles - gameboy.mmu.cycles;
        if mem_cycles != 0 && !previously_halted && !gameboy.halted {
            panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mmu.cycles)
        } else if mem_cycles == 1 {
            gameboy.mmu.cycle()
        } else {
            for _ in 0..mem_cycles {
                gameboy.mmu.cycle()
            }
        }
        gameboy.mmu.cycles = 0;
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

#[cfg(test)]
mod tests {
    use std::fs::{read, read_dir, DirEntry};
    use std::{env, panic};

    use std::io::Error;

    use crate::cartridge::Cartridge;
    use crate::{run_frame, Gameboy, MemoryManagementUnit};
    use image::io::Reader;
    use image::RgbaImage;
    use std::path::Path;
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_roms() -> Result<(), Error> {
        let (test_status_tx, test_status_rv) = channel();
        let _args: Vec<String> = env::args().collect();

        // panic::set_hook(Box::new(|_info| std::process::exit(1)));

        let all_tests = read_dir(env::current_dir().unwrap().join(Path::new("test_rom")))?;
        let all_tests: Vec<DirEntry> = all_tests
            .map(|entry| entry.unwrap())
            .filter(|entry| {
                let rom = String::from(entry.path().to_str().unwrap()).replace('\\', "/");
                let latest_img_path =
                    rom.replace("test_rom", "test_latest").replace('\\', "/") + ".png";

                let _latest_image = Path::new(&latest_img_path);

                if !rom.ends_with(".gb") {
                    println!("Skipping non ROM file: {rom}");
                    return false;
                }
                let _rom_vec = read(&rom).unwrap();

                true
            })
            .collect();

        let total = all_tests.len();
        for (idx, entry) in all_tests.into_iter().enumerate() {
            let tx_finish = test_status_tx.clone();
            thread::spawn(move || {
                const TEST_DURATION: u8 = 30;
                let rom = String::from(entry.path().to_str().unwrap()).replace('\\', "/");
                println!("Testing {}", rom);
                let rom_vec = read(&rom).unwrap();
                let cartridge = Cartridge::new(&rom_vec);

                let mem = MemoryManagementUnit::new(rom_vec, cartridge, None, &rom);
                let mut gameboy = Gameboy::new(mem);
                let mut tests_counter = 0;
                let r = rom.clone();
                let (tx, rx) = channel();

                thread::spawn(move || {
                    for i in 0..TEST_DURATION {
                        thread::sleep(Duration::from_secs(1));
                        println!("Saving screenshot #{i} for {r}");
                        if let Err(e) = tx.send(r.clone()) {
                            panic!("Panicked with {e} while saving screenshot #{i} for {r}")
                        };
                    }
                });
                'inner: loop {
                    if rx.try_recv().is_ok() {
                        tests_counter += 1;
                        if tests_counter >= TEST_DURATION - 1 {
                            break 'inner;
                        }

                        let map_pixel = |pixel: &u32| {
                            let pixels = pixel.to_be_bytes();
                            let a = pixels[0];
                            let r = pixels[1];
                            let g = pixels[2];
                            let b = pixels[3];
                            [r, g, b, a]
                        };
                        let pixels = gameboy
                            .mmu
                            .ppu
                            .screen
                            .iter()
                            .flat_map(map_pixel)
                            .collect::<Vec<u8>>();

                        let screenshot_path = rom.split('/').collect::<Vec<&str>>();
                        let img_name = *screenshot_path.last().unwrap();
                        let screenshot_path = screenshot_path[0..screenshot_path.len() - 2]
                            .join("/")
                            + "/test_output/"
                            + img_name
                            + ".png";
                        RgbaImage::from_raw(160, 144, pixels)
                            .unwrap()
                            .save(Path::new(&screenshot_path))
                            .unwrap();
                        let screenshot = Reader::open(screenshot_path.clone())
                            .unwrap()
                            .decode()
                            .unwrap();
                        let _screenshot = screenshot.as_bytes();
                        let _ok_image =
                            Reader::open(screenshot_path.clone().replace("test_output", "test_ok"));
                        let _latest_image = Reader::open(
                            screenshot_path
                                .clone()
                                .replace("test_output", "test_latest"),
                        );
                    }

                    run_frame(&mut gameboy, false, 0.0);
                }
                tx_finish.send(idx).unwrap();
            });
        }
        let mut count = 0;
        while count != total {
            match test_status_rv.recv() {
                Ok(_) => {
                    println!("Increased counter {count}/{total}");
                    count += 1
                }
                Err(e) => println!("Error receiving: {e}"),
            }
            if count == total {
                return Ok(());
            }
        }
        Err(Error::last_os_error())
    }
}
