extern crate core;

#[cfg(target_arch = "wasm32")]
use {
    leptos::js_sys::{Array, ArrayBuffer, Uint8Array},
    wasm_bindgen::{JsCast},
    wasm_bindgen::closure::Closure,
    wasm_bindgen_futures::JsFuture,
    web_sys::{console, HtmlInputElement, HtmlAnchorElement, HtmlDivElement, Blob, Request, RequestInit, Response, Url, window},
};

#[cfg(any(unix, windows))]
use {
    std::io::{Write},
    rand::Rng,
    rand::distributions::Uniform,
    std::fs::{read, write, File},
    winit::event::Event,
    winit::event::{WindowEvent::Focused},
    std::thread,
};

use std::collections::HashMap;

use gameboy::Gameboy;

use crate::mmu::MemoryManagementUnit;
use instant::{Duration, Instant};

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::Ordering::Relaxed;

use crate::cartridge::Cartridge;
use crate::register::Register;

use clap::{Parser, ValueEnum};
use cpal::traits::StreamTrait;

use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use pixels::wgpu::PresentMode;

use winit::dpi::LogicalSize;
use winit::event::VirtualKeyCode::{Back, Down, Escape, Left, Return, Right, Up, C, F, S, Z, P, M};
use winit::event::{VirtualKeyCode};

use winit::event_loop::EventLoop;
use winit::window::Fullscreen::Borderless;
use winit::window::{Window, WindowBuilder};
use winit_input_helper::WinitInputHelper;
use crate::SaveFile::{Bin, Json};
use crate::logger::Logger;

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
mod apu;

#[cfg(test)]
mod test;
mod mbc5;
mod logger;

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

    /// Use specified file format for saves
    #[clap(value_enum, long, default_value_t = SaveFile::Bin)]
    format: SaveFile,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum SaveFile {
    Json,
    Bin,
}

impl SaveFile {
    const FORMATS: [Self; 2] = [Json, Bin];

    fn extension(&self) -> &str {
        match self {
            Json => ".sav.json",
            Bin => ".sav.bin"
        }
    }

    fn save(&self, gameboy: &Gameboy) -> Vec<u8> {
        match self {
            Json => serde_json::to_vec(gameboy).unwrap(),
            Bin => bincode::serialize(gameboy).unwrap()
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn start_wasm(file: web_sys::File) {
    let event_loop = EventLoop::new();

    let window = setup_window(file.name()).build(&event_loop).unwrap();

    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.get_element_by_id("ironboy-canvas"))
        .and_then(|container| {
            use winit::platform::web::WindowExtWebSys;
            let canvas = &web_sys::Element::from(window.canvas());
            canvas.set_attribute("style", "width: 100%; height: 100%").unwrap();
            canvas.set_attribute("tabindex", "1").unwrap();
            canvas.set_attribute("id", "ironboy-screen").unwrap();
            canvas.set_attribute("style", "width: 100%; height: 100%").unwrap();
            container.append_child(canvas).ok()
        });

    window.set_inner_size(LogicalSize::new(240, 218));

    let pixels = setup_pixels(&window).await;
    file_callback(pixels, event_loop, Some(file)).await;
}

#[cfg(target_arch = "wasm32")]
async fn run() {
    let received = Arc::new(AtomicBool::new(false));
    let recv_file = {
        Closure::<dyn FnMut()>::wrap(Box::new(move || {
            let received = received.clone();
            let document = web_sys::window().unwrap().document().unwrap();
            let file = document.get_element_by_id("ironboy-input")
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap()
                .files()
                .unwrap()
                .item(0)
                .unwrap();
            web_sys::console::log_1(&format!("{}", file.name()).into());
            wasm_bindgen_futures::spawn_local(async move {
                if received.load(Relaxed) { return }
                received.store(true, Relaxed);
                Logger::info(format!("Receiving file: {:?}", file));
                start_wasm(file).await;
            })
        }))
    };
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("ironboy-input"))
        .and_then(|i| i.dyn_into::<HtmlInputElement>().ok())
        .and_then(|i| i.add_event_listener_with_callback("change", recv_file.as_ref().dyn_ref().unwrap()).ok());
    recv_file.forget(); // TODO: this leaks. I forgot how to get around that.

    if let Some(demo) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("ironboy-demo"))
        .and_then(|i| i.dyn_into::<HtmlDivElement>().ok()) {
        let run_demo = {
            Closure::<dyn FnMut()>::wrap(Box::new(move || {
                wasm_bindgen_futures::spawn_local(async move {
                    let mut opts = RequestInit::new();
                    opts.method("GET");
                    // opts.mode(RequestMode::Cors);
                    let url = format!("pocket.gb");
                    let request = Request::new_with_str_and_init(&url, &opts).unwrap();
                    let resp_value = JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request)).await.unwrap();
                    let resp: Response = resp_value.dyn_into().unwrap();

                    // Convert this other `Promise` into a rust `Future`.
                    let array_buffer: ArrayBuffer = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap().dyn_into::<>().unwrap();
                    let arr = Array::new();
                    arr.push(&array_buffer);
                    let file = web_sys::File::new_with_buffer_source_sequence(&arr, "demo.gb").unwrap();

                    start_wasm(file).await;
                })
            }))
        };
        demo.add_event_listener_with_callback("click", run_demo.as_ref().dyn_ref().unwrap()).ok();
        run_demo.forget();
    }
    web_sys::console::log_1(&"Loading IronBoy.".into());
}

#[cfg(target_arch = "wasm32")]
async fn file_callback(pixels: Pixels, event_loop: EventLoop<()>, file: Option<web_sys::File>) {
    let file = match file {
        Some(file) => file,
        None => return,
    };
    console::log_2(&"File:".into(), &file.name().into());
    let array_buffer = JsFuture::from(file.array_buffer()).await.unwrap();
    let data = Uint8Array::new(&array_buffer).to_vec();

    console::log_2(
        &"Got data".into(),
        &String::from_utf8_lossy(&data).into_owned().into(),
    );

    let name = file.name().replace(".sav.bin", "").replace(".sav.json", "");
    let gameboy = load_gameboy(pixels, file.name(), false, None, data);

    let doc = web_sys::window().unwrap().document().unwrap();
    doc.get_element_by_id("rom-selector")
        .unwrap()
        .set_attribute("style", "display: none")
        .unwrap();
    doc.get_element_by_id("ironboy-demo")
        .map(|d| d.set_attribute("style", "display: none"))
        .or(Some(Ok(Logger::error("#ironboy-demo not found"))));

    doc.get_element_by_id("ironboy-led")
        .unwrap()
        .set_attribute("style", "filter: brightness(1.5); transition: all 1.5s linear")
        .unwrap();
    run_event_loop(event_loop, gameboy, true, false, name, SaveFile::Bin);
}

#[cfg(target_arch = "wasm32")]
fn main_wasm() {
    console_error_panic_hook::set_once();
    wasm_rs_async_executor::single_threaded::block_on(run());
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    main_wasm();

    #[cfg(any(unix, windows))]
    main_desktop();
}

#[cfg(any(unix, windows))]
fn main_desktop() {
    let args = Args::parse();
    let rom_path = args.rom_file;

    let event_loop = EventLoop::new();
    let window = setup_window(rom_path.clone()).build(&event_loop).unwrap();
    let pixels = setup_pixels(&window);
    let rom = read(rom_path.clone()).expect("Unable to read ROM file");
    let gameboy = load_gameboy(pixels, rom_path.clone(), args.cold_boot, args.boot_rom, rom);

    run_event_loop(event_loop, gameboy, !args.fast, false, rom_path, args.format);
}

fn run_event_loop(
    event_loop: EventLoop<()>,
    mut gameboy: Gameboy,
    mut sleep: bool,
    mut muted: bool,
    rom_path: String,
    format: SaveFile,
) {
    let mut input = WinitInputHelper::new();

    let mut frames = 0.0;
    let start = Instant::now();

    let mut slowest_frame = Duration::from_nanos(0);

    let mut paused = false;
    if let Some(stream) = &gameboy.mmu.apu.stream {
        stream.play().unwrap();
    }

    let mut last_save = Instant::now();

    #[cfg(target_os = "macos")]
        let mut focus = (Instant::now(), true);

    #[cfg(target_arch = "wasm32")]
        let mut sleep_time = Duration::from_secs(0);
    #[cfg(target_arch = "wasm32")]
        let mut wait_time = Instant::now();
    #[cfg(target_arch = "wasm32")]
        let keymap = setup_virtual_pad();

    let mut previously_muted = false;
    event_loop.run(move |event, _target, control_flow| {
        let gameboy = &mut gameboy;
        input.update(&event);

        if input.key_released(P) {
            paused = !paused;
            if let Some(stream) = &gameboy.mmu.apu.stream {
                if paused { stream.pause().unwrap(); } else if !muted { stream.play().unwrap(); }
            }
        }

        if input.key_released(Escape) {
            Logger::info(format!(
                "Finished running at {} FPS average.\nSlowest frame took {:?}.\nSlowest render frame took {:?}.",
                frames / start.elapsed().as_secs_f64(),
                slowest_frame,
                gameboy.mmu.renderer.slowest
            ));
            control_flow.set_exit();
        }

        if let (Some(size), Some(p)) = (input.window_resized(), gameboy.mmu.renderer.pixels().as_mut()) {
            p.resize_surface(size.width, size.height).unwrap();
        }

        #[cfg(target_os = "wasm32")]
        if let Event::WindowEvent { event: Focused(false), .. } = event {
            paused = true;
        }

        #[cfg(target_os = "macos")]
        {
            if !paused && focus.1 && Instant::now() > focus.0 {
                // Save temporary dummy file to prevent throttling on Apple Silicon after focus change
                let dummy_data: Vec<u8> = rand::thread_rng().sample_iter(&Uniform::from(0..255)).take(0xFFFFFF).collect();

                write(rom_path.clone() + ".tmp", dummy_data).unwrap();
                focus.1 = false;
            }

            if let Event::WindowEvent { event: Focused(true), .. } = event {
                if !sleep {
                    focus = (Instant::now() + Duration::from_secs_f64(0.5), true);
                }
            }
        }

        if input.key_released(S) && last_save + Duration::from_secs(1) < Instant::now() {
            save_state(rom_path.clone(), gameboy, format);
            last_save = Instant::now();
        }

        if input.key_released(F) {
            sleep = !sleep;
        }

        if input.key_released(M) {
            muted = !muted;
        }

        if paused {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        if wait_time.elapsed() < sleep_time {
            return;
        } else {
            let keymap = keymap.clone();
            let run = run_frame(gameboy, sleep, &mut muted, Some(&input), Some(keymap));
            sleep_time = run.1;
            if slowest_frame < run.0 {
                slowest_frame = run.0;
            }
            wait_time = instant::Instant::now();
        }

        #[cfg(any(unix, windows))] {
            let (current_frame, sleep_time) = run_frame(gameboy, sleep, &mut false, Some(&input), None);
            thread::sleep(sleep_time);
            if slowest_frame < current_frame {
                slowest_frame = current_frame;
            }
        }

        if let Some(stream) = &gameboy.mmu.apu.stream {
            if muted && !previously_muted {
                previously_muted = true;
                stream.pause().unwrap();
            } else if !muted && previously_muted {
                previously_muted = false;
                stream.play().unwrap();
            }
        }

        frames += 1.0;
    });
}

fn run_frame(gameboy: &mut Gameboy, sleep: bool, mute: &mut bool, input: Option<&WinitInputHelper>, keymap: Option<Arc<Mutex<HashMap<&str, AtomicBool>>>>) -> (Duration, Duration) {
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
        }
        (0..mem_cycles).for_each(|_| gameboy.mmu.cycle(4));
        gameboy.mmu.cycles = 0;
    }

    let map_held = |buttons: [VirtualKeyCode; 4]| -> Vec<VirtualKeyCode> {
        buttons
            .iter()
            .filter(|&&b| input.map_or(false, |input| input.key_held(b)))
            .copied()
            .collect()
    };

    let action = [Z, C, Back, Return];
    let direction = [Up, Down, Left, Right];

    gameboy.mmu.joypad.held_action = map_held(action);
    gameboy.mmu.joypad.held_direction = map_held(direction);

    if let Some(keymap) = keymap {
        for (key, value) in keymap.lock().unwrap().iter() {
            if value.load(Relaxed) {
                let code = match *key {
                    "a" => Z,
                    "b" => C,
                    "select" => Back,
                    "start" => Return,
                    "up" => Up,
                    "left" => Left,
                    "right" => Right,
                    "down" => Down,
                    "speaker" => M,
                    _ => unreachable!()
                };
                if action.contains(&code) && !gameboy.mmu.joypad.held_action.contains(&code) {
                    gameboy.mmu.joypad.held_action.push(code);
                } else if direction.contains(&code) && !gameboy.mmu.joypad.held_direction.contains(&code) {
                    gameboy.mmu.joypad.held_direction.push(code);
                } else if code == M {
                    *mute = !*mute;
                    value.store(false, Relaxed);
                }
            }
        }
    }

    if !sleep {
        return (start.elapsed(), Duration::from_secs(0));
    }

    let expected = pin.1 + Duration::from_nanos(pin.0 * NANOS_PER_FRAME);

    let now = Instant::now();
    gameboy.pin = if now < expected {
        Some(pin)
    } else {
        None
    };

    (start.elapsed(), if now < expected { expected - now } else { Duration::from_secs(0) })
}

#[cfg(target_arch = "wasm32")]
fn setup_virtual_pad() -> Arc<Mutex<HashMap<&'static str, AtomicBool>>> {
    let keymap: Arc<Mutex<HashMap<&str, AtomicBool>>> = Arc::new(Mutex::new(HashMap::new()));

    let doc = window().unwrap().document().unwrap();

    let ids = [
        "a", "b", "up", "down", "left", "right", "start", "select"
    ];

    let km = keymap.clone();
    let toggle_speaker = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
        let km = &km.lock().unwrap();
        let state = km.get("speaker").unwrap();
        state.store(!state.load(Ordering::Relaxed), Ordering::Relaxed);
    });
    let speaker = doc.get_element_by_id("speaker").unwrap();
    speaker.add_event_listener_with_callback(
        "pointerdown",
        toggle_speaker.as_ref().unchecked_ref(),
    ).unwrap();
    toggle_speaker.forget();

    let elms = ids.map(|k| doc.get_element_by_id(k).unwrap());

    for id in ids {
        keymap.lock().unwrap().insert(id, AtomicBool::new(false));
    }

    keymap.lock().unwrap().insert("speaker", AtomicBool::new(false));

    elms.iter().enumerate().for_each(|(idx, elm)| {
        let km = keymap.clone();
        let pointer_enter = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            km
                .lock()
                .unwrap()
                .get(ids[idx])
                .unwrap()
                .store(true, Ordering::Relaxed);
        });

        let km = keymap.clone();
        let pointer_leave = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            km
                .lock()
                .unwrap()
                .get(ids[idx])
                .unwrap()
                .store(false, Ordering::Relaxed);
        });

        elm.add_event_listener_with_callback(
            "pointerenter",
            pointer_enter.as_ref().unchecked_ref(),
        ).unwrap();

        elm.add_event_listener_with_callback(
            "pointerleave",
            pointer_leave.as_ref().unchecked_ref(),
        ).unwrap();

        pointer_enter.forget();
        pointer_leave.forget();
    });
    keymap
}

fn save_state(rom_path: String, gameboy: &mut Gameboy, format: SaveFile) {
    Logger::info("Saving state.");

    let rom_path = SaveFile::FORMATS
        .iter()
        .map(SaveFile::extension)
        .fold(rom_path, |path, extension| path.replace(extension, ""))
        + format.extension();

    gameboy.mmu.save();

    let now = Instant::now();
    let save = format.save(gameboy);
    Logger::info(format!("Serialization took {}ms", now.elapsed().as_millis()));

    #[cfg(any(unix, windows))]
    thread::spawn(move || {
        let now = Instant::now();

        let mut save_file = File::create(&rom_path).unwrap();
        save_file.write_all(save.as_slice()).unwrap();

        Logger::info(format!("Save file {} successfully generated in {}ms.", rom_path, now.elapsed().as_millis()));
    });

    #[cfg(target_arch = "wasm32")]
    {
        window()
            .and_then(|w| w.document())
            .and_then(|d| d.create_element("a").ok())
            .and_then(|a| a.dyn_into::<HtmlAnchorElement>().ok())
            .and_then(|a| {
                let array = Array::new();
                let uarray = Uint8Array::new_with_length(save.len() as u32);
                uarray.copy_from(&save);
                array.push(&uarray);
                let blob = Blob::new_with_u8_array_sequence(&array);
                let object_url = Url::create_object_url_with_blob(&blob.unwrap());
                a.set_href(&object_url.unwrap());
                a.set_attribute("download", &rom_path.clone()).unwrap();
                a.click();
                Some(())
            });
    }
}

fn load_gameboy(
    pixels: Pixels,
    rom_path: String,
    cold_boot: bool,
    boot_rom: Option<String>,
    mut data: Vec<u8>,
) -> Gameboy {
    let mut gameboy = if rom_path.ends_with(".gb") || rom_path.ends_with(".gbc") {
        let cartridge = Cartridge::new(&data);
        let mem = MemoryManagementUnit::new(data, cartridge, boot_rom, Path::new(&rom_path));
        Gameboy::new(mem)
    } else {
        let format = if rom_path.ends_with(".json") {
            Json
        } else if rom_path.ends_with(".bin") {
            Bin
        } else {
            panic!("Unexpected file format for ROM save file: {}", rom_path);
        };

        let mut gb: Gameboy = match format {
            Json => serde_json::from_slice(data.as_mut()).unwrap(),
            Bin => bincode::deserialize(data.as_mut()).unwrap()
        };
        gb.init();
        gb
    };

    if cold_boot {
        gameboy.reg = Register::new(gameboy.mmu.boot_rom.is_some())
    }

    gameboy.mmu.renderer.set_pixels(pixels);
    gameboy.mmu.start();

    gameboy
}

#[cfg(target_arch = "wasm32")]
async fn setup_pixels(window: &Window) -> Pixels {
    let (width, height) = (WIDTH as u32, HEIGHT as u32);
    PixelsBuilder::new(width, height, SurfaceTexture::new(width, height, window))
        .present_mode(PresentMode::Fifo)
        .build_async()
        .await
        .unwrap()
}

#[cfg(any(unix, windows))]
fn setup_pixels(window: &Window) -> Pixels {
    let (width, height) = (WIDTH as u32, HEIGHT as u32);
    PixelsBuilder::new(width, height, SurfaceTexture::new(width, height, window))
        .present_mode(PresentMode::AutoNoVsync)
        .build()
        .unwrap()
}

fn setup_window(rom_path: String) -> WindowBuilder {
    WindowBuilder::new()
        .with_title(rom_path)
        .with_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_min_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_resizable(true)
        .with_visible(true)
        .with_fullscreen(Some(Borderless(None)))
}

const CYCLES_PER_FRAME: u16 = 17556;
const NANOS_PER_FRAME: u64 = 16742706;
