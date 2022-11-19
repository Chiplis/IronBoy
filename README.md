# feboy
A Gameboy emulator written in Rust as both a learning exercise and a love letter to the console that got me into gaming.

I decided to focus only on the original Game Boy (DMG) to achieve the highest degree of accuracy that I possibly can. The repository also contains more than 100 different test ROMs to verify correctness and detect any regressions.

---
## Building
```cargo build --release```

The emulator has been built and executed without issues on Windows, Linux and macOS.

---
## Testing
```cargo test --release```

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
      --threshold <THRESHOLD>  Minimum threshold to trigger sleep between frames [default: 0]
      --boot-rom <BOOT_ROM>    Use specified boot ROM
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
F -> Toggle frame limiter on/off
Esc -> Close
```

---
## Missing features

* Sound

* Full MBC support (as of now only MBC0, MBC1 and MBC3 have been implemented)
