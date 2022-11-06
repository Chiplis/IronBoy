use std::fs::{read, read_dir, DirEntry};
use std::{env, io};

use std::io::Error;

use crate::{run_frame, Gameboy, MemoryMap, WIDTH, HEIGHT};
use image::io::Reader;
use image::RgbaImage;
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use std::path::PathBuf;

#[test]
fn test_roms() -> Result<(), io::Error> {
    let (test_status_tx, test_status_rv) = channel();
    let args: Vec<String> = env::args().collect();

    let skip_known = args.contains(&"skip-known".to_owned());
    let skip_same = args.contains(&"skip-same".to_owned());

    let test_rom_dir = cwd().join("test_rom");

    let all_tests: Vec<DirEntry> = read_dir(test_rom_dir)?
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            let rom_path = entry.path();
            let rom_name = rom_path.file_name().unwrap().to_str().unwrap();

            let latest_img_path = rom_path
                .join(format!("../../test_latest/{rom_name}.png"));

            if skip_known && latest_img_path.exists() {
                println!("Skipping already tested ROM: {rom_name}");
                return false;
            }

            if rom_path.extension().unwrap_or_default().to_str().unwrap().to_lowercase() != "gb" {
                println!("Skipping non ROM file: {rom_name}");
                return false;
            }

            let rom_size = rom_path.metadata().unwrap().len();
            if rom_size > 32768 {
                println!("Still need to implement MBC for larger ROM's: {rom_name}");
                return false;
            }

            true
        })
        .collect();
    let total = all_tests.len();
    for (idx, entry) in all_tests.into_iter().enumerate() {
        let tx_finish = test_status_tx.clone();
        thread::spawn(move || {
            const TEST_DURATION: u8 = 30;
            let rom = entry.path();

            let rom_vec = read(&rom).unwrap();
            let mem = MemoryMap::new(&rom_vec, &rom.to_str().unwrap(), true, None);
            let mut gameboy = Gameboy::new(mem);
            println!("Beginning test loop");
            let mut tests_counter = 0;

            let r = rom.to_string_lossy().to_string();
            let (tx, rx) = std::sync::mpsc::channel();

            thread::spawn(move || {
                for i in 0..TEST_DURATION {
                    thread::sleep(Duration::from_secs(1));
                    println!("Saving screenshot #{i} for {r}");
                    if let Err(e) = tx.send(r.clone()) {
                        println!("Panicked with {e} while saving screenshot #{i} for {r}")
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
                        .mem
                        .ppu
                        .pixels
                        .iter()
                        .flat_map(map_pixel)
                        .collect::<Vec<u8>>();

                    let rom_name = rom.file_name().unwrap().to_str().unwrap();
                    let screenshot_output_path = rom.join(&format!("../../test_output/{rom_name}.png"));
                    let screenshot_ok_path = rom.join(&format!("../../test_ok/{rom_name}.png"));
                    let screenshot_latest_path = rom.join(&format!("../../test_latest/{rom_name}.png"));

                    let screenshot = image::load_from_memory(&pixels).unwrap();
                    let screenshot = screenshot.as_bytes();

                    RgbaImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
                        .unwrap()
                        .save(Path::new(&screenshot_output_path))
                        .unwrap();

                    let ok_image =
                        Reader::open(screenshot_ok_path);
                    let latest_image = Reader::open(screenshot_latest_path);
                    if ok_image.is_ok()
                        && ok_image.unwrap().decode().unwrap().as_bytes() == screenshot
                    {
                        println!(
                            "Ending {} test because result was confirmed as OK",
                            rom_name,
                        );
                        break 'inner;
                    }
                    if skip_same
                        && latest_image.is_ok()
                        && latest_image.unwrap().decode().unwrap().as_bytes() == screenshot
                    {
                        println!(
                            "Ending {} test because result was same as previously saved one",
                            rom_name,
                        );
                        break 'inner;
                    }
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

#[test]
fn test_regressions() -> Result<(), io::Error> {
    let mut regressions = vec![];
    for entry in read_dir(cwd().join("test_latest"))?.map(|entry| entry.unwrap()) {
        let path = entry.path();
        let img_name = path.file_name().unwrap().to_str().unwrap();
        let img_stem = path.file_stem().unwrap().to_str().unwrap();

        match check_regression(img_name) {
            Some(true) => regressions.push(img_stem.to_owned()),
            Some(false) => (),
            None => (),
        }
    }

    if !regressions.is_empty() {
        panic!("\nRegressions found:\n{}", regressions.join("\n"));
    }

    Ok(())
}

#[test]
fn test_differences() -> Result<(), io::Error> {
    let mut differences = vec![];
    for entry in read_dir(cwd().join(Path::new("test_latest")))? {
        if entry.is_err() { continue }

        let path = entry.unwrap().path();
        let img_name = path.file_name().unwrap().to_str().unwrap();
        let img_stem = path.file_stem().unwrap().to_str().unwrap();

        match check_regression(img_name) {
            // are different
            Some(true) => differences.push(img_stem.to_owned()),
            // are equal
            Some(false) => (),
            // unable to read / missing file
            None => differences.push(format!("MISSING: {}", img_stem)),
        }
    }
    if !differences.is_empty() {
        print!("Differences found:\n{}", differences.join("\n"));
    }
    Ok(())
}

fn check_regression(file_name: &str) -> Option<bool> {
    let images = (
        Reader::open(cwd().join(format!("test_output/{file_name}")))
            .map(|i| i.decode()),
        Reader::open(cwd().join(format!("test_latest/{file_name}")))
            .map(|i| i.decode()),
    );

    if let (Ok(Ok(ok_image)), Ok(Ok(latest_image))) = images {
        if ok_image.as_bytes() != latest_image.as_bytes() {
            Some(true)
        } else {
            Some(false)
        }
    } else {
        None
    }
}

#[inline]
fn cwd() -> PathBuf { std::env::current_dir().unwrap() }
