#[cfg(test)]
mod tests {
    use std::fs::{read, read_dir, DirEntry};
    use std::{env, panic};

    use std::io::Error;

    use crate::cartridge::Cartridge;
    use crate::{run_frame, Gameboy, MemoryManagementUnit};
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
                return if !rom.ends_with(".gb") {
                    println!("Skipping non ROM file: {rom}");
                    return false;
                } else {
                    true
                };
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

                let mem = MemoryManagementUnit::new(rom_vec, cartridge, None, Path::new(&rom));
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
                            [pixels[1], pixels[2], pixels[3], pixels[0]]
                        };
                        let pixels = gameboy
                            .mmu
                            .ppu
                            .screen
                            .iter()
                            .flat_map(map_pixel)
                            .collect();

                        let screenshot_path = rom.replace("test_rom", "test_output") + ".png";
                        RgbaImage::from_raw(160, 144, pixels)
                            .unwrap()
                            .save(Path::new(&screenshot_path))
                            .unwrap();
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