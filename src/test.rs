use std::ffi::OsStr;
use std::fs::{read, read_dir};
use std::env;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;

use image::RgbaImage;

use crate::cartridge::Cartridge;
use crate::{run_frame, Gameboy, MemoryManagementUnit, WIDTH, HEIGHT};

#[test]
fn test_roms() -> Result<(), Error> {
    let (test_status_tx, test_status_rv) = channel();
    // let _args: Vec<String> = env::args().collect();

    // panic::set_hook(Box::new(|_info| std::process::exit(1)));

    let cwd = env::current_dir().unwrap();

    let all_tests = read_dir(cwd.join("test_rom"))?;
    let all_tests: Vec<PathBuf> = all_tests
        .filter_map(|entry| {
            let output = match entry {
                Ok(entry) => match entry.metadata().map(|entry| entry.is_file()) {
                    Ok(true) => {
                        let path = entry.path();
                        match path.extension() {
                            Some(ext) if ext.to_ascii_lowercase() == "gb" => Ok(path),
                            Some(_) => Err(path),
                            None => Err(path),
                        }
                    },
                    Ok(false) => Err(entry.path()),
                    Err(_) => Err(entry.path()),
                },
                Err(_) => Err(PathBuf::new()),
            };
            match output {
                Ok(path) => Some(path),
                Err(path) => {
                    println!("Skipping non ROM file: {}", osstr_to_str(path.file_name()));
                    None
                }
            }
        })
        .collect();

    let total = all_tests.len();
    for (idx, rom) in all_tests.into_iter().enumerate() {
        let rom_filename = osstr_to_str(rom.file_name());
        let rom_output_png = cwd.join(format!("test_output/{}.png", rom_filename));

        let tx_finish = test_status_tx.clone();
        thread::spawn(move || {
            const TEST_DURATION: usize = 1200; // in frames

            println!("Testing {}", rom_filename);
            let rom_vec = read(rom.clone()).unwrap();
            let cartridge = Cartridge::new(&rom_vec);

            let mem = MemoryManagementUnit::new(rom_vec, cartridge, None, Path::new(&rom));
            let mut gameboy = Gameboy::new(mem);

            for _frame in 0..TEST_DURATION {
                run_frame(&mut gameboy, false, None);
            }

            println!("Saving screenshot for {rom_filename}");

            RgbaImage::from_raw(
                WIDTH as u32,
                HEIGHT as u32,
                gameboy.mmu.ppu.screen.to_vec()
            )
                .unwrap()
                .save(Path::new(&rom_output_png))
                .unwrap();

            tx_finish.send(idx).unwrap();
        });
    }
    let mut count = 0;
    while count < total {
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

#[inline]
fn osstr_to_str(item: Option<&OsStr>) -> String {
    item.unwrap().to_str().unwrap().to_string()
}
