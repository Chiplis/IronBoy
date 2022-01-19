use std::{env, thread};

use gameboy::Gameboy;

use crate::memory_map::MemoryMap;
use std::time::{Instant, Duration};

use std::fs::{read};

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
    let rom = read(rom_name).unwrap();
    let mem = MemoryMap::new(&rom, rom_name);

    let mut gameboy = Gameboy::new(mem);

    loop {
        run_frame(&mut gameboy);
    }
}

fn run_frame(gameboy: &mut Gameboy) {
    let mut elapsed_cycles = 0;
    const CYCLE_DURATION: f64 = 1.0_f64 / FREQUENCY as f64;
    let start = Instant::now();
    while elapsed_cycles < FREQUENCY / 60 {
        let previously_halted = gameboy.halted;
        let cycles = gameboy.cycle() as u16;
        elapsed_cycles += cycles as u32 * 4;
        let mem_cycles = cycles - gameboy.mem.micro_ops;
        if mem_cycles != 0 && !previously_halted && !gameboy.halted {
            panic!("Cycle count after considering reads/writes: mem_cycles {} | cycles: {} | micro_ops: {}", mem_cycles, cycles, gameboy.mem.micro_ops)
        } else if mem_cycles != 0 {
            for _ in 0..mem_cycles {
                gameboy.mem.micro_cycle();
            }
        }
        gameboy.mem.micro_ops = 0;
    }
    let cycles_time: f64 = CYCLE_DURATION * elapsed_cycles as f64;
    let sleep_time = cycles_time - start.elapsed().as_secs_f64();
    if sleep_time > 0.0 { thread::sleep(Duration::from_secs_f64(sleep_time)); }
}

#[cfg(test)]
mod tests {
    use std::fs::{read, read_dir};
    use std::{env, io};
    use std::path::Path;
    use std::thread;
    use std::time::{Duration};
    use image::{RgbaImage};
    use crate::{Gameboy, MemoryMap, run_frame};


    #[test]
    fn test_roms() -> Result<(), io::Error> {
        let (tx, rx) = std::sync::mpsc::channel();
        let args: Vec<String> = env::args().collect();
        let skip_known = args.contains(&"skip-known".to_owned());
        let skip_ok = args.contains(&"skip-ok".to_owned());
        for entry in read_dir(env::current_dir().unwrap().join(Path::new("tests")))? {
            let entry_path = entry.as_ref().unwrap().path();

            let p = &(entry_path.to_str().unwrap().replace("tests", "test_latest") + ".png");
            let latest_path = Path::new(p);
            if skip_known && latest_path.exists() {
                println!("Skipping already tested ROM: {}", entry.as_ref().unwrap().path().to_str().unwrap());
                continue;
            }

            let p = &(entry_path.to_str().unwrap().replace("tests", "test_ok") + ".png");
            let ok_path = Path::new(p);
            if skip_ok && ok_path.exists() {
                println!("Skipping already passing ROM: {}", entry.as_ref().unwrap().path().to_str().unwrap());
                continue;
            }

            let entry = entry?;
            let path = entry.path();
            let rom = String::from(path.to_str().unwrap());
            if !rom.ends_with(".gb") { continue; }

            let rom_vec = read(rom.clone()).unwrap();
            if rom_vec.len() > 32768 {
                println!("Still need to implement MBC for larger ROM's: {}", rom.clone());
                continue;
            }
            let mem = MemoryMap::new(&rom_vec, &rom);
            let mut gameboy = Gameboy::new(mem);

            let mut spawn = true;
            'inner: loop {
                if spawn {
                    let tx_clone = tx.clone();
                    thread::spawn(move || {
                        thread::sleep(Duration::from_secs(15));
                        tx_clone.send("").unwrap();
                    });
                    spawn = false;
                }

                for _ in rx.try_recv() {
                    let map_pixel = |pixel: &u32| {
                        let pixels = pixel.to_be_bytes();
                        let a = pixels[0];
                        let r = pixels[1];
                        let g = pixels[2];
                        let b = pixels[3];
                        [r, g, b, a]
                    };
                    let pixels = gameboy.mem.ppu.pixels.iter().flat_map(map_pixel).collect::<Vec<u8>>();
                    let rom_clone = rom.clone().replace("\\", "/");
                    let screenshot_path = rom_clone.as_str().split("/").collect::<Vec<&str>>();
                    let img_name = *screenshot_path.last().unwrap();
                    let screenshot_path = screenshot_path[0..screenshot_path.len() - 2].join("/") + "/test_output/" + img_name + ".png";
                    println!("{}", screenshot_path);
                    RgbaImage::from_raw(160, 144, pixels)
                        .unwrap()
                        .save(Path::new(&screenshot_path))
                        .unwrap();
                    break 'inner;
                }

                run_frame(&mut gameboy);
            }
        }
        Ok(())
    }

    #[test]
    fn test_regressions() -> Result<(), io::Error> {
        use image::io::Reader;

        let mut regressions = vec![];
        for entry in read_dir(env::current_dir().unwrap().join(Path::new("test_latest")))? {
            let p = {
                let path = entry.map(|e| e.path()).unwrap();
                path.to_str().unwrap().to_owned()
            };
            let path = p.split("\\").flat_map(|p| p.split("/")).collect::<Vec<&str>>();
            let directory = path[0..path.len() - 2].join("/");
            let img_name = path.last().unwrap();
            println!("{:?}", directory);
            let ok_image = Reader::open(directory.clone() + "/test_ok/" + img_name);
            let latest_image = Reader::open(directory + "/test_latest/" + img_name);
            if ok_image.is_err() { continue; }
            if ok_image.unwrap().decode().unwrap().as_bytes() != latest_image.unwrap().decode().unwrap().as_bytes() {
                regressions.push(img_name.replace(".png", ""));
            }
        }

        if !regressions.is_empty() {
            panic!("\nRegressions found:\n{}", regressions.join("\n"));
        }

        Ok(())
    }

    #[test]
    fn test_differences() -> Result<(), io::Error> {
        use image::io::Reader;

        let mut differences = vec![];
        for entry in read_dir(env::current_dir().unwrap().join(Path::new("test_latest")))? {
            let p = {
                let path = entry.map(|e| e.path()).unwrap();
                path.to_str().unwrap().to_owned()
            };
            let path = p.split("/").collect::<Vec<&str>>();
            let directory = path[0..path.len() - 2].join("/");
            let img_name = path.last().unwrap();

            let output_image = Reader::open(directory.clone() + "/test_output/" + img_name);
            let latest_image = Reader::open(directory + "/test_latest/" + img_name);

            if output_image.unwrap().decode().unwrap().as_bytes() != latest_image.unwrap().decode().unwrap().as_bytes() {
                differences.push(img_name.replace(".png", ""));
            }
        }
        if !differences.is_empty() {
            print!("Differences found:\n{}", differences.join("\n"));
        }
        Ok(())
    }
}