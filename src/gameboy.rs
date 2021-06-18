use std::ops::{Index, IndexMut};

use crate::memory_map::MemoryMap;
use crate::register::{ByteRegister, ConditionCode, FlagRegister, ProgramCounter, RegisterId, WordRegister, Register};
use crate::register::RegisterId::{A, B, C, D, E, H, L};
use crate::register::WordRegister::StackPointer;
use crate::interrupt::InterruptState::*;
use crate::instruction_fetcher::InstructionFetcher;
use crate::instruction::Command;
use crate::instruction::Command::*;
use std::cmp::max;
use crate::interrupt::InterruptId::{VBlankInt, StatInt, TimerInt, SerialInt, JoypadInt};
use crate::interrupt::InterruptId;

pub struct Gameboy {
    pub registers: Register,
    pub vram: [u8; 2 * 8 * 1024],
    pub ime_counter: i8,
    pub ime: bool,
    pub mem: MemoryMap,
    pub(crate) halted: bool
}

impl Gameboy {
    pub fn new(mem: MemoryMap) -> Self {
        Self {
            registers: Register::new(),
            mem,
            vram: [0; 2 * 8 * 1024],
            ime_counter: -1,
            ime: false,
            halted: false
        }
    }
}

impl Gameboy {
    #[deny(unreachable_patterns)]
    pub fn cycle(&mut self) -> u8 {
        let interrupt_cycles = if self.handle_interrupts() { 4 } else { 0 };
        if self.halted {
            self.halted = interrupt_cycles == 0;
            return if self.halted { 1 } else { 4 }
        }

        let instruction = InstructionFetcher::fetch_instruction(self.registers.pc.0, &self.registers, &self.mem);
        let (opcode, command) = (instruction.0, instruction.1);
        let line =  *self.mem.ppu.ly();
        println!("op: {} | pc: {} | sp: {} | a: {} b: {} c: {} d: {} e: {} h: {} l: {} | f: {} | ly: {}", opcode, self.registers.pc.0 + 1, self.registers.sp.to_address(), self[A].value, self[B].value, self[C].value, self[D].value, self[E].value, self[H].value, self[L].value, self.registers.flags.value(), line);

        self.registers.pc.0 += command.size() as u16;

        self.execute_command(command, interrupt_cycles)
    }

    fn execute_command(&mut self, command: Command, interrupt_cycles: u8) -> u8 {

        let hl = self.registers.hl();
        let mut branch_taken = true;

        match command {
            NOP => {}

            ADD_A_R8(ByteRegister { value: n, id: _ }) | ADD_A_U8(n) => {
                let (add, carry) = calc_with_carry(vec![self[A].value, n], &mut 0, |a, b| a.overflowing_add(b));
                self.registers.set_flags(add == 0, false, half_carry_8_add(self[A].value, n, 0), carry);
                self[A].value = add;
            }
            ADC_A_R8(ByteRegister { value: n, id: _ }) | ADC_A_U8(n) => {
                let carry = if self.registers.flags.c { 1 } else { 0 };
                let (add, new_carry) = calc_with_carry(vec![self[A].value, n, carry], &mut 0, |a, b| a.overflowing_add(b));
                self.registers.set_flags(add == 0, false, half_carry_8_add(self[A].value, n, carry), new_carry);
                self[A].value = add;
            }
            ADC_A_HL | ADD_A_HL => {
                let carry = if let ADD_A_HL = command { 1 } else { if self.registers.flags.c { 1 } else { 0 } };
                let (add, new_carry) = calc_with_carry(vec![self[A].value, self.mem[hl], carry], &mut 0, |a, b| a.overflowing_add(b));
                self.registers.set_flags(add == 0, false, half_carry_8_add(self[A].value, self.mem[hl], carry), new_carry);
                self[A].value = add;
            }
            AND_A_R8(ByteRegister { value: n, id: _ }) | AND_A_U8(n) => {
                self[A].value &= n;
                self.registers.set_flags(self[A].value == 0, false, true, false);
            }
            AND_A_HL => {
                self[A].value &= self.mem[hl];
                self.registers.set_flags(self[A].value == 0, false, true, false);
            }
            CP_A_R8(ByteRegister { value: n, id: _ }) | CP_A_U8(n) =>
                self.registers.set_flags(self[A].value == n, true, half_carry_8_sub(self[A].value, n, 0), n > self[A].value),
            CP_A_HL => {
                let n = self.mem[hl];
                self.registers.set_flags(self[A].value == n, true, half_carry_8_sub(self[A].value, n, 0), n > self[A].value);
            }

            DEC_R8(ByteRegister { value: _, id: id }) => {
                let reg = self[id].value;
                self[id].value = reg.wrapping_sub(1);
                let z = self[id].value == 0;
                self.registers.set_flags(z, true, half_carry_8_sub(reg, 1, 0), self.registers.flags.c);
            }

            INC_R8(ByteRegister { value: _, id: id }) => {
                let reg = self[id].value;
                self[id].value = reg.wrapping_add(1);
                let z = self[id].value == 0;
                let hc = half_carry_8_add(reg, 1, 0);
                self.registers.set_flags(z, false, hc, self.registers.flags.c);
            }
            OR_A_R8(ByteRegister { value: n, id: _ }) | OR_A_U8(n) => {
                self[A].value |= n;
                self.registers.set_flags(self[A].value == 0, false, false, false);
            }
            OR_A_HL => {
                self[A].value |= self.mem[hl];
                self.registers.set_flags(self[A].value == 0, false, false, false);
            }
            SUB_A_R8(ByteRegister { value: n, id: _ }) | SUB_A_U8(n) => {
                let (sub, c) = calc_with_carry(vec![self[A].value, n], &mut 0, |a, b| a.overflowing_sub(b));
                self[A].value = sub;
                self.registers.set_flags(self[A].value == 0, true, half_carry_8_sub(self[A].value, n, 0), c);
            }
            SBC_A_R8(ByteRegister { value: n, id: _ }) | SBC_A_U8(n) => {
                let carry = if self.registers.flags.c { 1 } else { 0 };
                let (sub, new_carry) = calc_with_carry(vec![self[A].value, n, carry], &mut 0, |a, b| a.overflowing_sub(b));
                self[A].value = sub;
                self.registers.set_flags(self[A].value == 0, true, half_carry_8_sub(self[A].value, n, carry), new_carry);
            }
            SBC_A_HL | SUB_A_HL => {
                let carry = if let SUB_A_HL = command { 1 } else { if self.registers.flags.c { 1 } else { 0 } };
                let (sub, new_carry) = calc_with_carry(vec![self[A].value, self.mem[hl], carry], &mut 0, |a, b| a.overflowing_sub(b));
                self[A].value = sub;
                self.registers.set_flags(self[A].value == 0, true, half_carry_8_sub(self[A].value, self.mem[hl], carry), new_carry);
            }
            XOR_A_R8(ByteRegister { value: n, id: _ }) | XOR_A_U8(n) => {
                self[A].value ^= n;
                self.registers.set_flags(self[A].value == 0, false, false, false);
            }
            XOR_A_HL => {
                self[A].value ^= self.mem[hl];
                self.registers.set_flags(self[A].value == 0, false, false, false);
            }
            ADD_HL_R16(reg) => {
                let hc = half_carry_16_add(hl.to_address(), reg.to_address(), 0);
                let (hl, carry) = hl.to_address().overflowing_add(reg.to_address());
                self.registers.set_word_register(hl, self.registers.hl());
                self.registers.set_flags(self.registers.flags.z, false, hc, carry);
            }
            DECH_HL => {
                let old = self.mem[hl];
                self.mem *= (hl, old.wrapping_sub(1));
                let hc = half_carry_8_sub(old, 1, 0);
                self.registers.set_flags(self.mem[hl] == 0, true, hc, self.registers.flags.c);
            }
            INCH_HL => {
                let old = self.mem[hl];
                self.mem *= (hl, old.wrapping_add(1));
                let hc = half_carry_8_add(old, 1, 0);
                self.registers.set_flags(self.mem[hl] == 0, false, hc, self.registers.flags.c);
            }
            DEC_R16(reg) => self.registers.set_word_register(reg.to_address().wrapping_sub(1), reg),
            INC_R16(reg) => self.registers.set_word_register(reg.to_address().wrapping_add(1), reg),
            RR_HL | RL_HL | RRC_HL | RLC_HL | RR_R8(_) | RL_R8(_) | RLA | RRA | RLC_R8(_) | RRC_R8(_) | RLCA | RRCA => {
                let mut value = match command {
                    RL_R8(r) | RR_R8(r) | RLC_R8(r) | RRC_R8(r) => self[r.id].value,
                    RLA | RRA | RLCA | RRCA => self[A].value,
                    RR_HL | RL_HL | RRC_HL | RLC_HL => self.mem[hl],
                    _ => panic!(),
                };
                let carry = match command {
                    RLC_R8(_) | RL_R8(_) | RLA | RLCA | RLC_HL | RL_HL => value & 128 != 0,
                    _ => value & 1 != 0,
                };
                let mask_condition = match command {
                    RRC_R8(_) | RRC_HL | RRCA | RLC_R8(_) | RLC_HL | RLCA => carry,
                    _ => self.registers.flags.c
                };
                let mask = if mask_condition {
                    match command {
                        RRC_HL | RRC_R8(_) | RRCA | RR_R8(_) | RRA | RR_HL => 128,
                        _ => 1
                    }
                } else { 0 };
                value = (match command {
                    RLC_R8(_) | RL_R8(_) | RLA | RLCA | RLC_HL | RL_HL => value << 1,
                    RRC_HL | RRC_R8(_) | RRCA | RR_R8(_) | RRA | RR_HL => value >> 1,
                    _ => panic!()
                }) | mask;
                let z = match command {
                    RLA | RRA | RLCA | RRCA => false,
                    _ => value == 0
                };
                match command {
                    RL_R8(r) | RR_R8(r) | RLC_R8(r) | RRC_R8(r) => self[r.id].value = value,
                    RLA | RRA | RLCA | RRCA => self[A].value = value,
                    RR_HL | RL_HL | RRC_HL | RLC_HL => self.mem *= (hl, value),
                    _ => panic!()
                };
                self.registers.set_flags(z, false, false, carry);
            }
            SRA_R8(_) | SLA_R8(_) | SRL_R8(_) | SRL_HL | SLA_HL | SRA_HL => {
                let mut value = match command {
                    SRL_HL | SRA_HL | SLA_HL => self.mem[hl],
                    SLA_R8(r) | SRA_R8(r) => self[r.id].value,
                    _ => panic!(),
                };
                let carry = match command {
                    SRA_R8(_) | SRA_HL => value & 1 != 0,
                    _ => value & 128 != 0
                };
                value = match command {
                    SRA_R8(_) | SRA_HL => ((value as i8) >> 1) as u8,
                    SRL_HL | SRL_R8(_) => value >> 1,
                    SLA_R8(_) | SLA_HL => ((value as i8) << 1) as u8,
                    _ => panic!()
                };
                match command {
                    SRL_HL | SRA_HL | SLA_HL => self.mem *= (hl, value),
                    SLA_R8(r) | SRA_R8(r) => self[r.id].value = value,
                    _ => panic!()
                };
                self.registers.set_flags(value == 0, false, false, carry);
            }
            BIT_U3_R8(_, _) | BIT_U3_HL(_) | RES_U3_R8(_, _) |
            RES_U3_HL(_) | SET_U3_R8(_, _) | SET_U3_HL(_) => {
                match command {
                    BIT_U3_R8(bit, ByteRegister { value: n, id: _ }) => self.registers.flags.z = n & bit.0 == 0,
                    BIT_U3_HL(bit) => self.registers.flags.z = self.mem[hl] & bit.0 == 0,
                    RES_U3_R8(bit, ByteRegister { value: _, id: id }) => {
                        self[id].value &= !bit.0
                    }
                    RES_U3_HL(bit) => {
                        self.mem *= (hl, self.mem[hl] & !bit.0)
                    }
                    SET_U3_R8(bit, ByteRegister { value: _, id: id }) => self[id].value |= bit.0,
                    SET_U3_HL(bit) => { self.mem *= (hl, self.mem[hl] | bit.0) }
                    _ => panic!()
                };
            }
            SWAP_R8(ByteRegister { value: n, id: id }) => {
                self.registers.set_flags(n == 0, false, false, false);
                self[id].value = n.rotate_left(4);
            }
            SWAP_HL => {
                self.registers.set_flags(hl.to_address() == 0, false, false, false);
                self.registers.set_word_register(hl.to_address().rotate_left(8), self.registers.hl());
            }
            LD_R8_R8(a, b) => self[a.id].value = b.value,
            LD_R8_U8(a, b) => self[a.id].value = b,
            LD_R16_U16(a, b) => self.registers.set_word_register(b, a),
            LD_HL_R8(ByteRegister { value: n, id: _ }) | LD_HL_N8(n) => { self.mem *= (hl, n); }
            LD_R8_HL(a) => {
                self[a.id].value = self.mem[hl]
            },
            LD_R16_A(n) => self.mem *= (n, self[A]),
            LDH_U16_A(n) => self.mem *= (n, self[A]),
            LDH_C_A => self.mem *= (self[C], self[A]),
            LD_A_U8(n) => self[A].value = n,
            LD_A_R16(n) => self[A].value = self.mem[n],
            LDH_A_U16(n) => self[A].value = self.mem[n],
            LDH_A_U8(n) => {
                let x = self.mem[n];
                self[A].value = x;
            }
            LDH_U8_A(n) => {
                if n == 0 {
                    let v = (self.mem[n] & 0xCF) | (self[A].value & 0x30);
                    self.mem *= (n, v)
                } else {
                    let x = self[A].value;
                    self.mem *= (n, x);
                }
            }
            LDH_HL_U8(n) => self.mem *= (hl, n),
            LDH_A_C => self[A].value = self.mem[self[C]],
            LD_A_HLD => {
                self.registers.set_word_register(hl.to_address().wrapping_sub(1), self.registers.hl());
                self[A].value = self.mem[hl];
            }
            LD_HLD_A => {
                self.registers.set_word_register(hl.to_address().wrapping_sub(1), self.registers.hl());
                self.mem *= (hl, self[A]);
            }
            LD_A_HLI => {
                self[A].value = self.mem[hl];
                self.registers.set_word_register(hl.to_address().wrapping_add(1), self.registers.hl());
            }
            LD_HLI_A => {
                self.mem *= (hl, self[A]);
                self.registers.set_word_register(hl.to_address().wrapping_add(1), self.registers.hl());
            }
            CALL_U16(n) => {
                let [lo, hi] = self.registers.pc.0.to_le_bytes();
                self.registers.sp = StackPointer(self.registers.sp.to_address() - 1);
                self.mem *= (self.registers.sp, hi);
                self.registers.sp = StackPointer(self.registers.sp.to_address() - 1);
                self.mem *= (self.registers.sp, lo);
                self.registers.pc.0 = n;
            }

            JP_HL => self.registers.pc.0 = self.registers.hl().to_address(),
            JP_U16(n) => self.registers.pc.0 = n,
            JR_I8(n) => self.registers.pc.0 = (self.registers.pc.0 as i16 + n as i16) as u16,
            CPL => {
                self[A].value = !self[A].value;
                self.registers.set_flags(self.registers.flags.z, true, true, self.registers.flags.c);
            }
            RET => {
                let lo = self.mem[self.registers.sp.to_address()];
                let hi = self.mem[self.registers.sp.to_address().wrapping_add(1)];
                self.registers.pc.0 = u16::from_le_bytes([lo, hi]);
                self.registers.set_word_register(self.registers.sp.to_address().wrapping_add(2), self.registers.sp);
            }
            RETI => {
                let lo = self.mem[self.registers.sp.to_address()];
                let hi = self.mem[self.registers.sp.to_address().wrapping_add(1)];
                self.registers.pc.0 = u16::from_le_bytes([lo, hi]);
                self.registers.set_word_register(self.registers.sp.to_address().wrapping_add(2), self.registers.sp);
                self.ime_counter = 1;
                self.ime = true;
            }
            RST(rst_vec) => {
                let [lo, hi] = self.registers.pc.0.to_le_bytes();
                self.registers.sp = StackPointer(self.registers.sp.to_address().wrapping_sub(1));
                self.mem *= (self.registers.sp, hi);
                self.registers.sp = StackPointer(self.registers.sp.to_address().wrapping_sub(1));
                self.mem *= (self.registers.sp, lo);
                self.registers.pc.0 = rst_vec as u16
            }
            ADD_HL_SP => {
                let (add, carry) = self.registers.hl().to_address().overflowing_add(self.registers.sp.to_address());
                self.registers.set_flags(add == 0, true, half_carry_16_add(self.registers.hl().to_address(), self.registers.sp.to_address(), 0), carry);
                self.registers.set_word_register(add, self.registers.hl());
            }
            ADD_SP_I8(n) | LD_HL_SP_I8(n) => {
                let (add, carry) = if n < 0 {
                    self.registers.sp.to_address().overflowing_sub((n as u8 & !0x80) as u16)
                } else {
                    self.registers.sp.to_address().overflowing_add((n as u8 & !0x80) as u16)
                };
                let half_carry = if n < 0 {
                    half_carry_16_sub(self.registers.sp.to_address(), (n as u8 & !0x80) as u16, 0)
                } else {
                    half_carry_16_add(self.registers.sp.to_address(), (n as u8 & !0x80) as u16, 0)
                };
                self.registers.set_flags(false, false, half_carry, carry);
                self.registers.set_word_register(add, if let ADD_SP_I8(n) = command { self.registers.sp } else { self.registers.hl() })
            }
            LD_U16_SP(n) => {
                let [lo, hi] = self.registers.sp.to_address().to_le_bytes();
                self.mem *= (n, lo);
                self.mem *= (n + 1, hi);
            }
            LD_U8_A(n) => self[A].value = n,
            LD_SP_HL => self.registers.set_word_register(self.registers.hl().to_address(), self.registers.sp),

            POP_R16(reg) => {
                match reg {
                    WordRegister::Double(ByteRegister { value: _, id: high }, ByteRegister { value: _, id: low }) => {
                        for id in &[low, high] {
                            self[*id].value = self.mem[self.registers.sp.to_address()];
                            self.registers.set_word_register(self.registers.sp.to_address().wrapping_add(1), self.registers.sp);
                        }
                    }
                    WordRegister::AccFlag(mut a, mut f) => {
                        self.registers.flags.set(self.mem[self.registers.sp.to_address()]);
                        self[A].value = self.mem[self.registers.sp.to_address().wrapping_add(1)];
                        self.registers.set_word_register(self.registers.sp.to_address().wrapping_add(2), self.registers.sp);
                    }

                    _ => panic!()
                }
            }
            PUSH_AF => {
                self.registers.set_word_register(self.registers.sp.to_address().wrapping_sub(1), self.registers.sp);
                self.mem *= (self.registers.sp, self[A]);
                self.registers.set_word_register(self.registers.sp.to_address().wrapping_sub(1), self.registers.sp);
                self.mem *= (self.registers.sp, self.registers.flags.value());
            }
            PUSH_R16(reg) => {
                match reg {
                    WordRegister::Double(ByteRegister { value: _, id: high }, ByteRegister { value: _, id: low }) => {
                        for id in &[high, low] {
                            self.registers.set_word_register(self.registers.sp.to_address().wrapping_sub(1), self.registers.sp);
                            let sp = self.registers.sp.to_address();
                            let value = self[*id].value;
                            self.mem *= (self.registers.sp, value);
                        }
                    }
                    _ => panic!()
                }
            }
            CCF => {
                self.registers.flags.n = false;
                self.registers.flags.h = false;
                self.registers.flags.c = !self.registers.flags.c;
            }
            DAA => {
                // note: assumes a is a uint8_t and wraps from 0xff to 0
                if !self.registers.flags.n {  // after an addition, adjust if (half-)carry occurred or if result is out of bounds
                    if self.registers.flags.c || self[A].value > 0x99 {
                        self[A].value += 0x60;
                        self.registers.flags.c = true;
                    }
                    if self.registers.flags.h || (self[A].value & 0x0f) > 0x09 {
                        self[A].value += 0x6;
                    }
                } else {
                    if self.registers.flags.c { self[A].value -= 0x60; }
                    if self.registers.flags.h { self[A].value -= 0x6; }
                }
                self.registers.flags.z = self[A].value == 0;
                self.registers.flags.h = false;
            }
            DI => { self.ime = false; }
            EI => { self.ime_counter = 2 }
            HALT => self.halted = true,
            SCF => {
                self.registers.flags.n = false;
                self.registers.flags.h = false;
                self.registers.flags.c = true;
            }

            RET_CC(cc) => if self.registers.cc_flag(cc) {
                let lo = self.mem[self.registers.sp.to_address()];
                let hi = self.mem[self.registers.sp.to_address().wrapping_add(1)];
                self.registers.pc.0 = u16::from_le_bytes([lo, hi]);
                self.registers.set_word_register(self.registers.sp.to_address().wrapping_add(2), self.registers.sp);
            } else { branch_taken = false }

            JP_CC_U16(cc, n) => if self.registers.cc_flag(cc) { self.registers.pc.0 = n; } else { branch_taken = false }

            JR_CC_I8(cc, n) => if self.registers.cc_flag(cc) { self.registers.pc.0 = (self.registers.pc.0 as i16 + n as i16) as u16 } else { branch_taken = false }

            CALL_CC_U16(cc, n) => if self.registers.cc_flag(cc) {
                let [lo, hi] = self.registers.pc.0.to_le_bytes();
                self.registers.sp = StackPointer(self.registers.sp.to_address() - 1);
                self.mem *= (self.registers.sp, hi);
                self.registers.sp = StackPointer(self.registers.sp.to_address() - 1);
                self.mem *= (self.registers.sp, lo);
                self.registers.pc.0 = n;
            } else { branch_taken = false }

            STOP => {}
        };
        command.cycles(branch_taken) + interrupt_cycles
    }

    fn handle_interrupts(&mut self) -> bool {
        self.ime_counter -= 1;
        if self.ime_counter == 0 {
            self.ime = true;
        }
        self.ime_counter = max(self.ime_counter, -1);
        if !self.ime { return false; }
        for interrupt_id in &[VBlankInt, StatInt, TimerInt, SerialInt, JoypadInt] {
            if self.trigger_interrupt(interrupt_id) {
                return true;
            }
        }
        return false;
    }

    fn trigger_interrupt(&mut self, interrupt_id: &InterruptId) -> bool {
        let state = self.mem.interrupt_handler.get_state(*interrupt_id);
        match state {
            Active => {
                self.ime = false;
                self.mem.interrupt_handler.set(vec![*interrupt_id], false);
                let [lo, hi] = self.registers.pc.0.to_le_bytes();
                self.registers.sp = StackPointer(self.registers.sp.to_address() - 1);
                self.mem *= (self.registers.sp, hi);
                self.registers.sp = StackPointer(self.registers.sp.to_address() - 1);
                self.mem *= (self.registers.sp, lo);
                self.registers.pc.0 = *interrupt_id as u16;
                true
            }
            Priority(priority_id) => self.trigger_interrupt(&priority_id),
            Inactive | Enabled | Requested => false
        }
    }
}

impl Index<RegisterId> for Gameboy {
    type Output = ByteRegister;

    fn index(&self, index: RegisterId) -> &Self::Output {
        &self.registers[index]
    }
}

impl IndexMut<RegisterId> for Gameboy {
    fn index_mut(&mut self, index: RegisterId) -> &mut Self::Output {
        &mut self.registers[index]
    }
}

fn calc_with_carry<T: Copy>(operands: Vec<T>, acc: &mut T, op: fn(T, T) -> (T, bool)) -> (T, bool) {
    let mut c = false;
    for x in operands {
        if !c {
            let res = op(*acc, x);
            *acc = res.0;
            c = res.1;
        } else {
            *acc = op(*acc, x).0
        }
    }
    (*acc, c)
}

fn half_carry_8_add(a: u8, b: u8, c: u8) -> bool { (((a & 0xF) + ((b + c) & 0xF)) & 0x10) == 0x10 }

fn half_carry_8_sub(a: u8, b: u8, c: u8) -> bool { (((a & 0xF).wrapping_sub(b.wrapping_add(c) & 0xF)) & 0x10) == 0x10 }

fn half_carry_16_add(a: u16, b: u16, c: u16) -> bool { ((a & 0xFF).wrapping_add((b.wrapping_add(c)) & 0xFF)) & 0x10 == 0x1000 }

fn half_carry_16_sub(a: u16, b: u16, c: u16) -> bool { ((a & 0xFF).wrapping_sub(b.wrapping_add(c) & 0xFF)) & 0x10 == 0x1000 }