extern crate core;

use std::thread;

use gameboy::Gameboy;

use crate::mmu::MemoryManagementUnit;
use instant::{Duration, Instant};

use std::io::{Read, Write};
use std::path::Path;

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

#[cfg(target_arch = "wasm32")]
use {
    leptos::js_sys::{Object, Reflect, Uint8Array},

    wasm_bindgen::{JsCast, UnwrapThrowExt},
    wasm_bindgen::closure::Closure,
    wasm_bindgen_futures::JsFuture,

    web_sys::{console, HtmlInputElement, ReadableStreamDefaultReader, HtmlElement},
};

#[cfg(any(unix, windows))]
use {
    rand::Rng,
    rand::distributions::Uniform,
    winit::event::{Event, WindowEvent},
    std::fs::{read, remove_file, write, File},
};

#[cfg(target_os="macos")]
use WindowEvent::Focused;

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
    let window = setup_window("silver.gb".to_string(), &event_loop);
    // Retrieve current width and height dimensions of browser client window
    let get_window_size = || {
        let client_window = web_sys::window().unwrap();
        LogicalSize::new(
            client_window.inner_width().unwrap().as_f64().unwrap(),
            client_window.inner_height().unwrap().as_f64().unwrap(),
        )
    };
    window.set_inner_size(get_window_size());
    let size = window.inner_size();
    let window_size = size;
    console::log_1(&"Setting up pixels2.".into());

    // Initialize winit window with current dimensions of browser client
    // Listen for resize event on browser client. Adjust winit window dimensions
    // on event trigger
    let closure = Closure::wrap(Box::new(|_e: web_sys::Event| {
        // Retrieve current width and height dimensions of browser client window
        let get_window_size = || {
            console::log_1(&"Getting windows size.".into());
            let client_window = web_sys::window().unwrap();
            LogicalSize::new(
                client_window.inner_width().unwrap().as_f64().unwrap(),
                client_window.inner_height().unwrap().as_f64().unwrap(),
            )
        };
        // TODO: Figure out how to handle resize
        // let size = get_window_size();
        // &window.set_inner_size(size);
    }) as Box<dyn FnMut(_)>);
    let client_window = web_sys::window().unwrap();
    client_window
        .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    console::log_1(&"Finishing pixels setup.".into());

    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body: HtmlElement| -> Option<()> {
            use winit::platform::web::WindowExtWebSys;
            let canvas = window.canvas();
            canvas.set_attribute("style", "width: 640px; height: 576px;").unwrap();

            let canvas = &web_sys::Element::from(window.canvas());

            body.append_child(&canvas).ok();
            Some(())
        })
        .unwrap();

    let pixels = setup_pixels(&window).await;
    file_callback(pixels, event_loop, Some(file)).await;
}

#[cfg(target_arch = "wasm32")]
async fn run() {
    web_sys::console::log_1(&"Setting up pixels1.".into());
    let w = web_sys::window();
    w
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body| {
            let input = web_sys::window().expect("WINDOW").document().unwrap()
                .create_element("input")
                .expect_throw("Create input")
                .dyn_into::<HtmlInputElement>()
                .unwrap();

            input
                .set_attribute("type", "file")
                .expect_throw("Set input type file");

            input
                .set_attribute("id", "iron-boy")
                .expect_throw("Set input id");

            let recv_file = {
                Closure::<dyn FnMut()>::wrap(Box::new(move || {
                    let document = web_sys::window().unwrap().document().unwrap();
                    let file = web_sys::Document::get_element_by_id(&document, "iron-boy")
                        .unwrap()
                        .dyn_into::<HtmlInputElement>()
                        .unwrap()
                        .files()
                        .unwrap()
                        .item(0)
                        .unwrap();
                    wasm_bindgen_futures::spawn_local(async move {
                        start_wasm(file).await;
                    })
                }))
            };
            input
                .add_event_listener_with_callback("change", recv_file.as_ref().dyn_ref().unwrap())
                .expect_throw("Listen for file upload");
            recv_file.forget(); // TODO: this leaks. I forgot how to get around that.

            body.append_child(&input).ok()
        })
        .expect("couldn't append canvas and input to document body");
    web_sys::console::log_1(&"Loading IronBoy.".into());
}

#[cfg(target_arch = "wasm32")]
async fn file_callback(pixels: Pixels, event_loop: EventLoop<()>, file: Option<web_sys::File>) {
    let file = match file {
        Some(file) => file,
        None => return,
    };
    console::log_2(&"File:".into(), &file.name().into());
    let reader = file
        .stream()
        .get_reader()
        .dyn_into::<ReadableStreamDefaultReader>()
        .expect_throw("Reader is reader");
    let mut data = Vec::new();
    loop {
        let chunk = JsFuture::from(reader.read())
            .await
            .expect_throw("Read")
            .dyn_into::<Object>()
            .unwrap();
        // ReadableStreamReadResult is somehow wrong. So go by hand. Might be a web-sys bug.
        let done = Reflect::get(&chunk, &"done".into()).expect_throw("Get done");
        if done.is_truthy() {
            break;
        }
        let chunk = Reflect::get(&chunk, &"value".into())
            .expect_throw("Get chunk")
            .dyn_into::<Uint8Array>()
            .expect_throw("bytes are bytes");
        let data_len = data.len();
        data.resize(data_len + chunk.length() as usize, 255);
        chunk.copy_to(&mut data[data_len..]);
    }
    console::log_2(
        &"Got data".into(),
        &String::from_utf8_lossy(&data).into_owned().into(),
    );

    let gameboy = load_gameboy_wasm(pixels, data);

    run_event_loop(event_loop, gameboy, true, false, false, "".to_string(), SaveFile::Json);
}

#[cfg(target_arch = "wasm32")]
fn load_gameboy_wasm(pixels: Pixels, data: Vec<u8>) -> Gameboy {
    console::log_1(&"Loading Cartridge".into());
    let cartridge = Cartridge::new(&data);
    console::log_1(&"Loading Memory".into());
    let mem = MemoryManagementUnit::new(data, cartridge, None, Path::new(""));
    console::log_1(&"Loading Gameboy".into());
    let mut gb = Gameboy::new(mem);
    gb.mmu.renderer.set_pixels(pixels);
    gb.mmu.start();
    gb
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
    let window = setup_window(rom_path.clone(), &event_loop);
    let pixels = setup_pixels(&window);
    let gameboy = load_gameboy(pixels, rom_path.clone(), args.cold_boot, args.boot_rom);

    run_event_loop(event_loop, gameboy, !args.fast, false, args.save_on_exit, rom_path, args.format);
}

fn run_event_loop(
    event_loop: EventLoop<()>,
    mut gameboy: Gameboy,
    mut sleep: bool,
    mut muted: bool,
    save_on_exit: bool,
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

    #[cfg(target_os = "macos")]
        let mut focus = (Instant::now(), true);

    event_loop.run(move |event, _target, control_flow| {

        let gameboy = &mut gameboy;
        input.update(&event);
        frames += 1.0;

        if input.key_released(P) {
            paused = !paused;
        }

        if input.key_released(Escape) {
            #[cfg(any(unix, windows))]
            exit_emulator(save_on_exit, rom_path.clone(), gameboy, format);
            println!(
                "Finished running at {} FPS average.\nSlowest frame took {:?}.\nSlowest render frame took {:?}.",
                frames / start.elapsed().as_secs_f64(),
                slowest_frame,
                gameboy.mmu.renderer.slowest
            );
            control_flow.set_exit();
        }

        if let (Some(size), Some(p)) = (input.window_resized(), gameboy.mmu.renderer.pixels().as_mut()) {
            p.resize_surface(size.width, size.height).unwrap();
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

        if input.key_released(S) {
            save_state(rom_path.clone(), gameboy, format);
        }

        if input.key_released(F) {
            sleep = !sleep;
            println!("Changed fast mode to {}", !sleep);
        }

        if input.key_released(M) {
            muted = !muted;
            if let Some(stream) = &gameboy.mmu.apu.stream {
                if muted { stream.pause().unwrap(); } else { stream.play().unwrap(); }
            }
        }

        if paused {
            return;
        }

        let current_frame = run_frame(gameboy, sleep, Some(&input));

        if slowest_frame < current_frame {
            slowest_frame = current_frame
        }
    });
}

fn run_frame(gameboy: &mut Gameboy, sleep: bool, input: Option<&WinitInputHelper>) -> Duration {
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

    gameboy.mmu.joypad.held_action = map_held([Z, C, Back, Return]);
    gameboy.mmu.joypad.held_direction = map_held([Up, Down, Left, Right]);

    if !sleep {
        return start.elapsed();
    }

    let expected = pin.1 + Duration::from_nanos(pin.0 * NANOS_PER_FRAME);

    let now = Instant::now();
    gameboy.pin = if now < expected {
        #[cfg(any(unix, windows))]
        thread::sleep(expected - now);

        // TODO: Implement frame limiting in WASM

        Some(pin)
    } else {
        None
    };

    start.elapsed()
}

fn save_state(rom_path: String, gameboy: &mut Gameboy, format: SaveFile) {
    println!("Saving state.");

    let rom_path = SaveFile::FORMATS
        .iter()
        .map(SaveFile::extension)
        .fold(rom_path, |path, extension| path.replace(extension, ""))
        + format.extension();

    gameboy.mmu.save();

    let now = Instant::now();
    let save = format.save(gameboy);
    println!("Serialization took {}ms", now.elapsed().as_millis());

    #[cfg(any(unix, windows))]
    thread::spawn(move || {
        let now = Instant::now();

        let mut save_file = File::create(&rom_path).unwrap();
        save_file.write_all(save.as_slice()).unwrap();

        println!("Save file {} successfully generated in {}ms.", rom_path, now.elapsed().as_millis());
    });
}

#[cfg(any(unix, windows))]
fn exit_emulator(save: bool, rom_path: String, gameboy: &mut Gameboy, format: SaveFile) {
    if save {
        save_state(rom_path.clone(), gameboy, format);
    }
    let tmp = rom_path + ".tmp";
    let tmp = Path::new(&tmp);
    if tmp.exists() {
        remove_file(tmp).unwrap();
    }
}

#[cfg(any(unix, windows))]
fn load_gameboy(
    pixels: Pixels,
    rom_path: String,
    cold_boot: bool,
    boot_rom: Option<String>,
) -> Gameboy {
    let mut gameboy = if rom_path.ends_with(".gb") || rom_path.ends_with(".gbc") {
        let rom = read(rom_path.clone()).expect("Unable to read ROM file");
        let cartridge = Cartridge::new(&rom);
        let mem = MemoryManagementUnit::new(rom, cartridge, boot_rom, Path::new(&rom_path));
        Gameboy::new(mem)
    } else {
        let save_file = &mut vec![];
        let format = if rom_path.ends_with(".json") {
            Json
        } else if rom_path.ends_with(".bin") {
            Bin
        } else {
            panic!("Unexpected file format for ROM save file: {}", rom_path);
        };

        File::open(rom_path)
            .unwrap()
            .read_to_end(save_file)
            .unwrap();
        let mut gb: Gameboy = match format {
            Json => serde_json::from_slice(save_file).unwrap(),
            Bin => bincode::deserialize(save_file.as_slice()).unwrap()
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

fn setup_window(rom_path: String, event_loop: &EventLoop<()>) -> Window {
    WindowBuilder::new()
        .with_title(rom_path)
        .with_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_min_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_resizable(true)
        .with_visible(true)
        .with_fullscreen(Some(Borderless(None)))
        .build(event_loop)
        .unwrap()
}

const CYCLES_PER_FRAME: u16 = 17556;
const NANOS_PER_FRAME: u64 = 16742706;
