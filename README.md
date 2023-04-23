![image psd(11)](https://user-images.githubusercontent.com/25803723/230508071-c5738223-f67d-46e5-8121-12f1ef99ae60.png)

# IronBoy
A Gameboy emulator written in Rust as both a learning exercise and a love letter to the console that got me into gaming.

I decided to focus only on the original Game Boy (DMG) to achieve the highest degree of accuracy that I possibly can. The repository also contains more than 100 different test ROMs to verify correctness and detect any regressions.

---
## Building
```cargo build --release```
```trunk build --release --filehash=false```
Download `trunk` from https://trunkrs.dev/ and run a local webserver in the generated /dist folder if you want to run the WASM version. 
The emulator has been built and executed without issues on Windows, Linux and macOS.

---
## Testing
```cargo test --release```
```trunk build --release --filehash=false && cp iron_boy.css dist```

Install
This should execute all available test ROMs and save the rendered output for each of them in the ```test_output``` folder.

---
## Running
```
Usage:
  cargo run --release -- [OPTIONS] <ROM_FILE>

Arguments:
  <ROM_FILE>  GameBoy ROM file to input

Options:
      --headless               Runs the emulator without a backing window, used during test execution
      --cold-boot              Boot title screen even when opening save file
      --fast                   Start emulator with unlocked framerate
      --save-on-exit           Automatically save state before exiting emulator
      --boot-rom <BOOT_ROM>    Use specified boot ROM
      --format <FORMAT>        Use specified file format for saves [default: bin] [possible values: json, bin]
  -h, --help                   Print help information
  -V, --version                Print version information
```
---
## Controls
```
Z -> A
C -> B
Enter (PC) / Return (Mac) -> Start
Backspace (PC) / Delete (Mac) -> Select

S -> Save
P -> Pause
F -> Toggle frame limiter
M -> Toggle sound
R -> Reset
Esc -> Close
```

---
## Missing features

* ~Sound~ - Credits to [@maxwalley](https://github.com/maxwalley)

* Full MBC support (as of now only MBC0, MBC1 and MBC3 have been implemented)
