use std::env;
use std::ffi::OsStr;
use std::time::Duration;
use std::thread;
use std::sync::mpsc::channel;
use std::path::PathBuf;
use std::panic;
use std::io::Error;
use std::fs::{read, read_dir, DirEntry};
use image::io::Reader;
use image::RgbaImage;

use crate::HEIGHT;
use crate::WIDTH;
use crate::{ run_frame, Gameboy, cartridge::Cartridge, mmu::MemoryManagementUnit };

#[inline]
fn cwd() -> PathBuf { env::current_dir().unwrap() }

#[test]
fn test_roms() -> Result<(), Error> {
    let (test_status_tx, test_status_rv) = channel();
    let _args: Vec<String> = env::args().collect();

    panic::set_hook(Box::new(|_info| std::process::exit(1)));

    let test_rom_dir = cwd().join("test_rom");

    let all_tests: Vec<DirEntry> = read_dir(test_rom_dir)?
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            let rom_path = entry.path();
            let rom_name = rom_path.file_stem().unwrap().to_str().unwrap();
            let rom_filename = rom_path.file_name().unwrap().to_str().unwrap();

            let _latest_img_path = rom_path
                .join(format!("../../test_latest/{rom_name}.png"));

            if rom_path.extension() != Some(OsStr::new("gb")) {
                println!("Skipping non ROM file: {rom_filename}");
                return false;
            }
            
            if read(&rom_path).is_ok() {
                true
            } else {
                println!("Failed reading ROM file: {rom_filename}");
                false
            }
        })
        .collect();

    let total = all_tests.len();
    for (idx, entry) in all_tests.into_iter().enumerate() {
        let tx_finish = test_status_tx.clone();
        thread::spawn(move || {
            const TEST_DURATION: u8 = 30;
            let rom = entry.path();
            println!("Testing {}", rom.file_name().unwrap().to_str().unwrap());
            let rom_vec = read(&rom).unwrap();
            let cartridge = Cartridge::new(&rom_vec);

            let mem = MemoryManagementUnit::new(rom_vec, cartridge, None, &rom);
            let mut gameboy = Gameboy::new(mem);
            let mut tests_counter = 0;
            let r = rom.clone();
            let rom_name = r.file_name().unwrap().to_str().unwrap().to_owned();
            let (tx, rx) = channel();

            thread::spawn(move || {
                for i in 0..TEST_DURATION {
                    thread::sleep(Duration::from_secs(1));
                    println!("Saving screenshot #{i} for {rom_name}");
                    if let Err(e) = tx.send(r.clone()) {
                        panic!("Panicked with {e} while saving screenshot #{i} for {rom_name}")
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

                    let rom_name = rom.file_name().unwrap().to_str().unwrap();
                    let screenshot_output_path = rom.join(&format!("../../test_output/{rom_name}.png"));
                    let screenshot_ok_path = rom.join(&format!("../../test_ok/{rom_name}.png"));
                    let screenshot_latest_path = rom.join(&format!("../../test_latest/{rom_name}.png"));

                    RgbaImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
                        .unwrap()
                        .save(&screenshot_output_path)
                        .unwrap();
                    let screenshot = Reader::open(&screenshot_output_path)
                        .unwrap()
                        .decode()
                        .unwrap();
                    let _screenshot = screenshot.as_bytes();
                    let _ok_image =
                        Reader::open(&screenshot_ok_path);
                    let _latest_image = Reader::open(&screenshot_latest_path);
                }

                run_frame(&mut gameboy, false);
            }
            tx_finish.send(idx).unwrap();
        });
    }
    let mut count = 0;
    while count != total {
        match test_status_rv.recv() {
            Ok(_) => {
                count += 1;
                println!("Increased counter {count}/{total}");
            }
            Err(e) => println!("Error receiving: {e}"),
        }
        if count == total {
            return Ok(());
        }
    }
    Err(Error::last_os_error())
}
