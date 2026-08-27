use std::env;
use std::ffi::OsStr;
use std::fs::{read, read_dir};
use std::io::Error;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;

use image::RgbaImage;

use crate::cartridge::Cartridge;
use crate::{run_frame, Gameboy, MemoryManagementUnit, HEIGHT, WIDTH};
use crate::logger::Logger;

#[test]
fn test_roms() -> Result<(), Error> {
    let (test_status_tx, test_status_rv) = channel();

    panic::set_hook(Box::new(|info| {
        eprintln!("{info}");
        std::process::exit(1);
    }));

    let all_tests = read_dir("test_rom")?;
    let filters: Vec<String> = env::var("IRONBOY_TEST_ROM")
        .unwrap_or_default()
        .split(',')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
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
                    }
                    Ok(false) => Err(entry.path()),
                    Err(_) => Err(entry.path()),
                },
                Err(_) => Err(PathBuf::new()),
            };
            match output {
                Ok(path) => Some(path),
                Err(path) => {
                    Logger::error(format!("Skipping non ROM file: {}", osstr_to_str(path.file_name())));
                    None
                }
            }
        })
        .filter(|path| {
            filters.is_empty()
                || filters
                    .iter()
                    .any(|name| path.file_name() == Some(OsStr::new(name)))
        })
        .filter(|path| {
            let filename = osstr_to_str(path.file_name());
            let rom = read(path).unwrap();
            let cgb_only = rom.get(0x143) == Some(&0xC0);
            let non_dmg_model = filename == "mgb_oam_dma_halt_sprites.gb";
            if cgb_only || non_dmg_model {
                Logger::info(format!("Skipping non-DMG ROM: {filename}"));
            }
            !cgb_only && !non_dmg_model
        })
        .collect();

    let total = all_tests.len();
    let worker_count = thread::available_parallelism()
        .map(|workers| workers.get())
        .unwrap_or(1);
    let mut count = 0;

    for chunk in all_tests.chunks(worker_count) {
        for rom in chunk.iter().cloned() {
            let rom_filename = osstr_to_str(rom.file_name());
            let rom_output_png = format!("test_output/{rom_filename}.png");
            let tx_finish = test_status_tx.clone();

            thread::spawn(move || {
                let test_duration = env::var("IRONBOY_TEST_FRAMES")
                    .ok()
                    .and_then(|frames| frames.parse().ok())
                    .unwrap_or(2400);

                Logger::info(format!("Testing {rom_filename}"));
                let rom_vec = read(rom.clone()).unwrap();
                let cartridge = Cartridge::new(&rom_vec);

                let mem = MemoryManagementUnit::new(rom_vec, cartridge, None, Path::new(&rom));
                let mut gameboy = Gameboy::new(mem);
                for _frame in 0..test_duration {
                    run_frame(&mut gameboy, Arc::new(AtomicBool::new(false)), None);
                }

                Logger::info(format!("Saving screenshot for {rom_filename}"));

                let actual = RgbaImage::from_raw(
                    WIDTH as u32,
                    HEIGHT as u32,
                    gameboy.mmu.ppu.screen.to_vec(),
                )
                .unwrap();
                actual.save(Path::new(&rom_output_png)).unwrap();

                if env::var_os("IRONBOY_SKIP_IMAGE_ASSERTS").is_none() {
                    let expected_path = format!("test_ok/{rom_filename}.png");
                    let expected = image::open(&expected_path)
                        .unwrap_or_else(|_| panic!("Missing expected image: {expected_path}"))
                        .to_rgba8();
                    if actual != expected {
                        panic!("Screenshot mismatch for {rom_filename}");
                    }
                }

                tx_finish.send(rom_filename).unwrap();
            });
        }

        for _ in chunk {
            match test_status_rv.recv() {
                Ok(_) => {
                    count += 1;
                    Logger::info(format!("Finished test {count}/{total}"));
                }
                Err(e) => Logger::error(format!("Error executing test: {e}")),
            }
        }
    }

    Ok(())
}

#[inline]
fn osstr_to_str(item: Option<&OsStr>) -> String {
    item.unwrap().to_str().unwrap().to_string()
}
