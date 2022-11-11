use std::ops::{Index, IndexMut};

use crate::instruction::Command::*;
use crate::instruction_fetcher::InstructionFetcher;
use crate::interrupt::IE_ADDRESS;
use crate::interrupt::IF_ADDRESS;
use crate::memory_map::MemoryMap;
use crate::register::RegisterId::*;
use crate::register::WordRegister::{ProgramCounter, StackPointer};
use crate::register::{ByteRegister, Register, RegisterId, WordRegister};
use std::cmp::max;

use crate::instruction::InstructionOperand::{OpByte, OpHL, OpRegister};
use crate::instruction::{Command, InstructionOperand};
use crate::interrupt::InterruptId;
use crate::interrupt::InterruptId::{Input, Serial, Stat, Timing, VBlank};

pub struct Gameboy {
    pub reg: Register,
    pub ei_counter: i8,
    pub ime: bool,
    pub mem: MemoryMap,
    pub halted: bool,
    counter: usize,
    bugged_pc: Option<WordRegister>,
}

impl Gameboy {
    pub fn new(mem: MemoryMap) -> Self {
        Self {
            reg: Register::new(mem.boot_rom.is_some()),
            mem,
            ei_counter: -1,
            ime: false,
            halted: false,
            bugged_pc: None,
            counter: 0,
        }
    }
}

impl Gameboy {
    #[deny(unreachable_patterns)]
    pub fn cycle(&mut self) -> u8 {
        let interrupt_cycles = if self.handle_interrupts() { 5 } else { 0 };

        if self.halted {
            self.halted = interrupt_cycles == 0;
            if self.halted
                && !self.ime
                && self.mem.read_without_cycle(IE_ADDRESS)
                    & self.mem.read_without_cycle(IF_ADDRESS)
                    & 0x1F
                    != 0
            {
                self.halted = false;
            }
            return 1 + interrupt_cycles;
        }

        if interrupt_cycles != 0 {
            return interrupt_cycles;
        }

        let instruction =
            InstructionFetcher::fetch_instruction(self.reg.pc.value(), &self.reg, &mut self.mem);
        let (_, command) = (instruction.0, instruction.1);
        /*
        let line = self.mem.ppu.ly();
        let _log = format!(
            "op:0x{:02x}|pc:{}|sp:{}|a:{}|b:{}|c:{}|d:{}|e:{}|h:{}|l:{}|f:{}|ly:{}|lt:{}",
            opcode,
            self.reg.pc.value() + 1,
            self.reg.sp.value(),
            self[A].value,
            self[B].value,
            self[C].value,
            self[D].value,
            self[E].value,
            self[H].value,
            self[L].value,
            self.reg.flags.value(),
            line,
            self.mem.ppu.last_ticks
        );
        println!("{}", log);
        println!("{:?}", command);
        */
        self.set_pc(self.reg.pc.value() + command.size() as u16, false);

        self.execute_instruction(command)
    }

    fn execute_instruction(&mut self, command: Command) -> u8 {
        let command_cycles = self.handle_command(command);

        match self.bugged_pc {
            Some(ProgramCounter(pc)) => {
                self.mem.memory.remove(pc as usize);
                if pc < self.reg.pc.value() {
                    self.set_pc(self.reg.pc.value() - 1, false);
                }
            }
            None => {}
            _ => panic!(),
        }

        self.bugged_pc = None;

        if !self.ime
            && self.halted
            && self.mem.read_without_cycle(IE_ADDRESS)
                & self.mem.read_without_cycle(IF_ADDRESS)
                & 0x1F
                != 0
        {
            self.halted = false;
            self.bugged_pc = Some(self.reg.pc);
            let x = self.mem.read(self.reg.pc);
            self.mem.memory.insert(self.reg.pc.value() as usize, x);
        }
        if command != Halt {
            command_cycles
        } else {
            self.mem.cycles as u8
        }
    }

    fn get_op(&mut self, op: InstructionOperand) -> u8 {
        match op {
            OpByte(n) => n,
            OpRegister(id) => self[id].value,
            OpHL => self.mem.read(self.reg.hl()),
        }
    }

    fn handle_interrupts(&mut self) -> bool {
        self.ei_counter -= 1;
        if self.ei_counter == 0 {
            self.ime = true;
        }
        self.ei_counter = max(self.ei_counter, -1);
        if !self.ime {
            return false;
        }
        self.trigger_interrupt(VBlank)
            || self.trigger_interrupt(Stat)
            || self.trigger_interrupt(Timing)
            || self.trigger_interrupt(Serial)
            || self.trigger_interrupt(Input)
    }

    fn trigger_interrupt(&mut self, interrupt_id: InterruptId) -> bool {
        if self.mem.interrupt_handler.triggered(interrupt_id) {
            self.micro_cycle();
            self.micro_cycle();
            self.ime = false;
            self.mem.interrupt_handler.unset(interrupt_id);
            let [lo, hi] = self.reg.pc.value().to_le_bytes();
            self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
            self.mem.write(self.reg.sp, hi);
            self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
            self.mem.write(self.reg.sp, lo);
            self.set_pc(interrupt_id as u16, true);
            true
        } else {
            false
        }
    }

    fn handle_command(&mut self, command: Command) -> u8 {
        let hl = self.reg.hl();
        let mut branch_taken = true;

        match command {
            JrCcI8(cc, _) | JpCcU16(cc, _) | RetCc(cc) | CallCcU16(cc, _) => {
                if self.reg.cc_flag(cc) {
                    self.micro_cycle();
                }
            }
            _ => {}
        }
        match command {
            Nop => {}

            AddA(op) => {
                let n = self.get_op(op);
                let (add, carry) =
                    calc_with_carry(vec![self[A].value, n, 0], |a, b| a.overflowing_add(b));
                self.reg.set_flags(
                    add == 0,
                    false,
                    half_carry_8_add(self[A].value, n, 0),
                    carry,
                );
                self[A].value = add;
            }

            AdcA(op) => {
                let carry = u8::from(self.reg.flags.c);
                let n = self.get_op(op);
                let (add, new_carry) =
                    calc_with_carry(vec![self[A].value, n, carry], |a, b| a.overflowing_add(b));
                self.reg.set_flags(
                    add == 0,
                    false,
                    half_carry_8_add(self[A].value, n, carry),
                    new_carry,
                );
                self[A].value = add;
            }

            AndA(op) => {
                self[A].value &= self.get_op(op);
                self.reg.set_flags(self[A].value == 0, false, true, false);
            }

            CpA(op) => {
                let n = self.get_op(op);

                self.reg.set_flags(
                    self[A].value == n,
                    true,
                    half_carry_8_sub(self[A].value, n, 0),
                    n > self[A].value,
                )
            }

            DecR8(id) => {
                let reg = self[id].value;
                self[id].value = reg.wrapping_sub(1);
                let z = self[id].value == 0;
                self.reg
                    .set_flags(z, true, half_carry_8_sub(reg, 1, 0), self.reg.flags.c);
            }

            IncR8(id) => {
                let reg = self[id].value;
                self[id].value = reg.wrapping_add(1);
                let z = self[id].value == 0;
                let hc = half_carry_8_add(reg, 1, 0);
                self.reg.set_flags(z, false, hc, self.reg.flags.c);
            }
            OrA(op) => {
                self[A].value |= self.get_op(op);
                self.reg.set_flags(self[A].value == 0, false, false, false);
            }

            SubA(op) => {
                let n = self.get_op(op);
                let (sub, c) =
                    calc_with_carry(vec![self[A].value, n, 0], |a, b| a.overflowing_sub(b));
                self.reg
                    .set_flags(sub == 0, true, half_carry_8_sub(self[A].value, n, 0), c);
                self[A].value = sub;
            }
            SbcA(op) => {
                let n = self.get_op(op);
                let carry = u8::from(self.reg.flags.c);
                let (sub, new_carry) =
                    calc_with_carry(vec![self[A].value, n, carry], |a, b| a.overflowing_sub(b));
                self.reg.set_flags(
                    sub == 0,
                    true,
                    half_carry_8_sub(self[A].value, n, carry),
                    new_carry,
                );
                self[A].value = sub;
            }

            XorA(op) => {
                self[A].value ^= self.get_op(op);
                self.reg.set_flags(self[A].value == 0, false, false, false);
            }

            AddHlR16(reg) => {
                let hc = half_carry_16_add(hl.value(), reg.value(), 0);
                let (hl, carry) = hl.value().overflowing_add(reg.value());
                self.set_word_register_with_micro_cycle(hl, self.reg.hl());
                self.reg.set_flags(self.reg.flags.z, false, hc, carry);
            }

            DechHl => {
                let old = self.mem.read(hl);
                self.mem.write(hl, old.wrapping_sub(1));
                let hc = half_carry_8_sub(old, 1, 0);
                self.reg
                    .set_flags(old.wrapping_sub(1) == 0, true, hc, self.reg.flags.c);
            }

            InchHl => {
                let old = self.mem.read(hl);
                self.mem.write(hl, old.wrapping_add(1));
                let hc = half_carry_8_add(old, 1, 0);
                self.reg
                    .set_flags(old.wrapping_add(1) == 0, false, hc, self.reg.flags.c);
            }

            DecR16(reg) => {
                self.mem.corrupt_oam(reg);
                self.set_word_register_with_micro_cycle(reg.value().wrapping_sub(1), reg)
            }

            IncR16(reg) => {
                self.mem.corrupt_oam(reg);
                self.set_word_register_with_micro_cycle(reg.value().wrapping_add(1), reg)
            }

            Rr(op, small) | Rl(op, small) | Rrc(op, small) | Rlc(op, small) => {
                let mut value = self.get_op(op);
                let carry = if let Rlc(..) | Rl(..) = command {
                    value & 128 != 0
                } else {
                    value & 1 != 0
                };
                let mask_condition = if let Rrc(..) | Rlc(..) = command {
                    carry
                } else {
                    self.reg.flags.c
                };
                let mask = if mask_condition {
                    if let Rr(..) | Rrc(..) = command {
                        128
                    } else {
                        1
                    }
                } else {
                    0
                };

                value = (if let Rr(..) | Rrc(..) = command {
                    value >> 1
                } else {
                    value << 1
                }) | mask;

                let z = !small && value == 0;

                match op {
                    OpRegister(id) => self[id].value = value,
                    OpHL => self.mem.write(hl, value),
                    _ => panic!(),
                };
                self.reg.set_flags(z, false, false, carry);
            }

            Sra(op) | Sla(op) | Srl(op) => {
                let mut value = self.get_op(op);
                let carry = value & if let Sla(_) = command { 128 } else { 1 } != 0;

                value = if let Sra(_) = command {
                    (value as i8 >> 1) as u8
                } else if let Srl(_) = command {
                    value >> 1
                } else {
                    ((value as i8) << 1) as u8
                };

                match op {
                    OpHL => self.mem.write(hl, value),
                    OpRegister(id) => self[id].value = value,
                    _ => panic!(),
                };

                self.reg.set_flags(value == 0, false, false, carry);
            }

            BitU3(bit, op) => {
                self.reg.flags.z = (self.get_op(op) & bit.0) ^ bit.0 == bit.0;
                self.reg.flags.n = false;
                self.reg.flags.h = true;
            }

            ResU3R8(bit, id) => self[id].value &= !bit.0,

            ResU3Hl(bit) => {
                let x = self.mem.read(hl);
                self.mem.write(hl, x & !bit.0)
            }

            SetU3R8(bit, id) => self[id].value |= bit.0,

            SetU3Hl(bit) => {
                let x = self.mem.read(hl);
                self.mem.write(hl, x | bit.0)
            }

            SwapR8(id) => {
                self.reg.set_flags(self[id].value == 0, false, false, false);
                self[id].value = self[id].value.rotate_left(4);
            }

            SwapHl => {
                let x = self.mem.read(hl);
                self.mem.write(hl, x.rotate_left(4));
                self.reg.set_flags(x == 0, false, false, false);
            }

            LdR8R8(a, b) => self[a].value = self[b].value,

            LdR8U8(a, b) => self[a].value = b,
            LdR16U16(a, b) => self.set_word_register(b, a),
            LdHlR8(id) => {
                self.mem.write(hl, self[id].value);
            }
            LdR8Hl(id) => self[id].value = self.mem.read(hl),
            LdR16A(n) => self.mem.write(n, self[A]),
            LdhU16A(n) => self.mem.write(n, self[A]),
            LdhCA => self.mem.write(self[C], self[A]),
            LdAU8(n) => self[A].value = n,
            LdAR16(n) => self[A].value = self.mem.read(n),
            LdhAU16(n) => self[A].value = self.mem.read(n),
            LdhAU8(n) => {
                self.counter += 1;
                let x = self.mem.read(n);
                self[A].value = x;
            }
            LdhU8A(n) => {
                self.mem.write(n, self[A].value);
            }
            LdhHlU8(n) => self.mem.write(hl, n),
            LdhAC => self[A].value = self.mem.read(self[C]),
            LdHldA => {
                /*
                TODO
                 Figure out if OAM corruption bug happens,
                 or if it gets ignored due to the Write + IncDec
                 */
                self.set_word_register(hl.value().wrapping_sub(1), self.reg.hl());
                self.mem.write(hl, self[A]);
            }
            LdHliA => {
                /*
                TODO
                 Figure out if OAM corruption bug happens,
                 or if it gets ignored due to the Write + IncDec
                 */
                self.mem.write(hl, self[A]);
                self.set_word_register(hl.value().wrapping_add(1), self.reg.hl());
            }
            LdAHli => {
                self.mem.corrupt_oam(hl);
                self[A].value = self.mem.read(hl);
                self.set_word_register(hl.value().wrapping_add(1), self.reg.hl());
            }
            LdAHld => {
                self.mem.corrupt_oam(hl);
                self.set_word_register(hl.value().wrapping_sub(1), self.reg.hl());
                self[A].value = self.mem.read(hl);
            }
            CallU16(n) => {
                self.micro_cycle();
                let [lo, hi] = self.reg.pc.value().to_le_bytes();
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem.write(self.reg.sp, hi);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem.write(self.reg.sp, lo);
                self.set_pc(n, false);
            }

            JpHl => self.set_pc(self.reg.hl().value(), false),
            JpU16(n) => self.set_pc(n, true),
            JrI8(n) => self.set_pc((self.reg.pc.value() as i16 + n as i16) as u16, true),
            Cpl => {
                self[A].value = !self[A].value;
                self.reg
                    .set_flags(self.reg.flags.z, true, true, self.reg.flags.c);
            }
            Ret => {
                let lo = self.mem.read(self.reg.sp);
                let hi = self.mem.read(self.reg.sp.value().wrapping_add(1));
                self.set_pc(u16::from_le_bytes([lo, hi]), true);
                self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
            }
            Reti => {
                let lo = self.mem.read(self.reg.sp);
                let hi = self.mem.read(self.reg.sp.value().wrapping_add(1));
                self.set_pc(u16::from_le_bytes([lo, hi]), true);
                self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
                self.ei_counter = 1;
                self.ime = true;
            }
            Rst(rst_vec) => {
                let [lo, hi] = self.reg.pc.value().to_le_bytes();
                self.set_pc(rst_vec as u16, true);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem.write(self.reg.sp, hi);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem.write(self.reg.sp, lo);
            }
            AddSpI8(n) | LdHlSpI8(n) => {
                let a = self.reg.sp.value();
                let b = n as i8 as i16 as u16;
                let h = (a & 0x000F) + (b & 0x000F) > 0x000F;
                let c = (a & 0x00FF) + (b & 0x00FF) > 0x00FF;
                self.reg.set_flags(false, false, h, c);
                if let AddSpI8(_) = command {
                    self.micro_cycle()
                }
                self.set_word_register_with_micro_cycle(
                    a.wrapping_add(b),
                    if let AddSpI8(_) = command {
                        self.reg.sp
                    } else {
                        self.reg.hl()
                    },
                )
            }
            LdU16Sp(n) => {
                let [lo, hi] = self.reg.sp.value().to_le_bytes();
                self.mem.write(n, lo);
                self.mem.write(n + 1, hi);
            }
            LdSpHl => self.set_word_register_with_micro_cycle(self.reg.hl().value(), self.reg.sp),

            PopR16(reg) => match reg {
                WordRegister::Double(
                    ByteRegister { value: _, id: high },
                    ByteRegister { value: _, id: low },
                ) => {
                    self.mem.corrupt_oam(self.reg.sp);
                    self[low].value = self.mem.read(self.reg.sp);
                    self.set_word_register(self.reg.sp.value().wrapping_add(1), self.reg.sp);
                    self[high].value = self.mem.read(self.reg.sp);
                    self.set_word_register(self.reg.sp.value().wrapping_add(1), self.reg.sp);
                }
                WordRegister::AccFlag(..) => {
                    self.mem.corrupt_oam(self.reg.sp);
                    self.reg.flags.set(self.mem.read(self.reg.sp));
                    self[A].value = self.mem.read(self.reg.sp.value().wrapping_add(1));
                    self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
                }

                _ => panic!(),
            },
            PushAf => {
                self.micro_cycle();
                self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                self.mem.write(self.reg.sp, self[A]);
                self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                self.mem.write(self.reg.sp, self.reg.flags.value());
            }
            PushR16(reg) => {
                self.mem.corrupt_oam(self.reg.sp);
                self.micro_cycle();
                match reg {
                    WordRegister::Double(
                        ByteRegister { value: _, id: high },
                        ByteRegister { value: _, id: low },
                    ) => {
                        self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                        let value = self[high].value;
                        self.mem.write(self.reg.sp, value);
                        self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                        let value = self[low].value;
                        self.mem.write(self.reg.sp, value);
                    }
                    _ => panic!(),
                }
            }
            Ccf => {
                self.reg.flags.n = false;
                self.reg.flags.h = false;
                self.reg.flags.c = !self.reg.flags.c;
            }
            Daa => {
                // note: assumes a is a uint8_t and wraps from 0xff to 0
                if !self.reg.flags.n {
                    // after an addition, adjust if (half-)carry occurred or if result is out of bounds
                    if self.reg.flags.c || self[A].value > 0x99 {
                        self[A].value = self[A].value.wrapping_add(0x60);
                        self.reg.flags.c = true;
                    }
                    if self.reg.flags.h || (self[A].value & 0x0f) > 0x09 {
                        self[A].value = self[A].value.wrapping_add(0x6);
                    }
                } else {
                    if self.reg.flags.c {
                        self[A].value = self[A].value.wrapping_sub(0x60);
                    }
                    if self.reg.flags.h {
                        self[A].value = self[A].value.wrapping_sub(0x6);
                    }
                }
                self.reg.flags.z = self[A].value == 0;
                self.reg.flags.h = false;
            }
            DisableInterrupt => self.ime = false,
            EnableInterrupt => self.ei_counter = 2,
            Halt => self.halted = true,
            Scf => {
                self.reg.flags.n = false;
                self.reg.flags.h = false;
                self.reg.flags.c = true;
            }

            RetCc(cc) => {
                if self.reg.cc_flag(cc) {
                    let lo = self.mem.read(self.reg.sp);
                    let hi = self.mem.read(self.reg.sp.value().wrapping_add(1));
                    self.set_pc(u16::from_le_bytes([lo, hi]), false);
                    self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
                } else {
                    branch_taken = false
                }
                self.micro_cycle();
            }

            JpCcU16(cc, n) => {
                if self.reg.cc_flag(cc) {
                    self.set_pc(n, false)
                } else {
                    branch_taken = false
                }
            }

            JrCcI8(cc, n) => {
                if self.reg.cc_flag(cc) {
                    self.set_pc((self.reg.pc.value() as i16 + n as i16) as u16, false)
                } else {
                    branch_taken = false
                }
            }

            CallCcU16(cc, n) => {
                if self.reg.cc_flag(cc) {
                    let [lo, hi] = self.reg.pc.value().to_le_bytes();
                    self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                    self.mem.write(self.reg.sp, hi);
                    self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                    self.mem.write(self.reg.sp, lo);
                    self.set_pc(n, false);
                } else {
                    branch_taken = false
                }
            }

            Stop => {}
        };
        command.cycles(branch_taken)
    }

    fn micro_cycle(&mut self) {
        self.mem.cycle();
    }

    fn set_pc(&mut self, value: u16, trigger_cycle: bool) {
        if trigger_cycle {
            self.mem.corrupt_oam(self.reg.pc.value());
        }
        self.reg.pc = ProgramCounter(value);
        if trigger_cycle {
            self.micro_cycle()
        }
    }

    fn set_word_register(&mut self, value: u16, reg: WordRegister) {
        self.reg.set_word_register(value, reg, &mut self.mem);
    }

    fn set_word_register_with_micro_cycle(&mut self, value: u16, reg: WordRegister) {
        self.reg
            .set_word_register_with_callback(value, reg, |mem| mem.cycle(), &mut self.mem);
    }
}

impl Index<RegisterId> for Gameboy {
    type Output = ByteRegister;

    fn index(&self, index: RegisterId) -> &Self::Output {
        &self.reg[index]
    }
}

impl IndexMut<RegisterId> for Gameboy {
    fn index_mut(&mut self, index: RegisterId) -> &mut Self::Output {
        &mut self.reg[index]
    }
}

fn calc_with_carry<T: Copy>(operands: Vec<T>, op: fn(T, T) -> (T, bool)) -> (T, bool) {
    let mut c = false;
    let mut acc = operands[0];
    for x in operands[1..].iter() {
        if !c {
            let res = op(acc, *x);
            acc = res.0;
            c = res.1;
        } else {
            acc = op(acc, *x).0
        }
    }
    (acc, c)
}

fn half_carry_8_add(a: u8, b: u8, c: u8) -> bool {
    (a & 0xF) + (b & 0xF) + c > 0xF
}

fn half_carry_8_sub(a: u8, b: u8, c: u8) -> bool {
    (a & 0x0F) < (b & 0x0F) + c
}

fn half_carry_16_add(a: u16, b: u16, c: u16) -> bool {
    (a & 0x07FF) + (b & 0x07FF) + c > 0x07FF
}
