use std::ops::{Index, IndexMut};

use crate::memory_map::MemoryMap;
use crate::register::{ByteRegister, RegisterId, DoubleRegisterId, Register, ProgramCounter};
use crate::register::RegisterId::*;
use crate::register::DoubleRegisterId::StackPointer;
use crate::interrupt::InterruptState::*;
use crate::interrupt::IF_ADDRESS;
use crate::interrupt::IE_ADDRESS;
use crate::instruction_fetcher::InstructionFetcher;
use crate::instruction::Command::*;
use std::cmp::max;
use crate::instruction::Command;
use crate::interrupt::InterruptId::{VBlankInt, StatInt, TimerInt, SerialInt, JoypadInt};
use crate::interrupt::{InterruptId};

pub struct Gameboy {
    pub reg: Register,
    pub ei_counter: i8,
    pub ime: bool,
    pub mem: MemoryMap,
    pub halted: bool,
    bugged_pc: Option<ProgramCounter>,
}

impl Gameboy {
    pub fn new(mem: MemoryMap) -> Self {
        Self {
            reg: Register::new(),
            mem,
            ei_counter: -1,
            ime: false,
            halted: false,
            bugged_pc: None,
        }
    }
}

impl Gameboy {
    #[deny(unreachable_patterns)]
    pub fn cycle(&mut self) -> u8 {
        let interrupt_cycles = if self.handle_interrupts() { 5 } else { 0 };

        if self.halted {
            self.halted = interrupt_cycles == 0;
            if self.halted && !self.ime {
                if self.mem.read_mem(IE_ADDRESS as u16, false)
                    & self.mem.read_mem(IF_ADDRESS as u16, false)
                    & 0x1F != 0 {
                    self.halted = false;
                }
            }
            return 1 + interrupt_cycles;
        }

        if interrupt_cycles != 0 {
            return interrupt_cycles;
        }

        let instruction = InstructionFetcher::fetch_instruction(self.reg.pc.0, &self.reg, &mut self.mem);
        let (opcode, command) = (instruction.0, instruction.1);
        let line = self.mem.ppu.ly();
        let _log = format!("op:0x{:02x}|pc:{}|sp:{}|a:{}|b:{}|c:{}|d:{}|e:{}|h:{}|l:{}|f:{}|ly:{}|lt:{}", opcode, self.reg.pc.0 + 1, self.reg.sp.value(), self[A].value, self[B].value, self[C].value, self[D].value, self[E].value, self[H].value, self[L].value, self.reg.flags.value(), line, self.mem.ppu.last_ticks);
        //println!("{}", log);
        //println!("{:?}", command);
        self.reg.pc.0 += command.size() as u16;

        self.execute_instruction(command)
    }

    fn execute_instruction(&mut self, command: Command) -> u8 {
        let command_cycles = self.handle_command(command);

        match self.bugged_pc {
            Some(ProgramCounter(pc)) => {
                self.mem.memory.remove(pc as usize);
                if pc < self.reg.pc.0 {
                    self.reg.pc.0 -= 1
                }
            }
            None => {}
        }

        self.bugged_pc = None;

        if !self.ime && self.halted && self.mem.read_mem(IE_ADDRESS as u16, false) & self.mem.read_mem(IF_ADDRESS as u16, false) & 0x1F != 0 {
            self.halted = false;
            self.bugged_pc = Some(self.reg.pc);
            let x = self.mem.read(self.reg.pc.0);
            self.mem.memory.insert(self.reg.pc.0 as usize, x);
        }
        if command != HALT { command_cycles } else {
            self.mem.micro_ops as u8
        }
    }

    fn handle_interrupts(&mut self) -> bool {
        self.ei_counter -= 1;
        if self.ei_counter == 0 {
            self.ime = true;
        }
        self.ei_counter = max(self.ei_counter, -1);
        if !self.ime { return false; }
        for interrupt_id in self.get_interrupts() {
            if self.trigger_interrupt(&interrupt_id) {
                return true;
            }
        }
        false
    }

    fn get_interrupts(&self) -> [InterruptId; 5] { [VBlankInt, StatInt, TimerInt, SerialInt, JoypadInt] }

    fn trigger_interrupt(&mut self, interrupt_id: &InterruptId) -> bool {
        let state = self.mem.interrupt_handler.get_state(*interrupt_id);
        match state {
            Active => {
                self.micro_cycle();
                self.micro_cycle();
                self.ime = false;
                self.mem.interrupt_handler.set(vec![*interrupt_id], false);
                let [lo, hi] = self.reg.pc.0.to_le_bytes();
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, hi);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, lo);
                self.set_pc(*interrupt_id as u16, true);
                true
            }
            Priority(priority_id) => self.trigger_interrupt(&priority_id),
            Inactive | Enabled | Requested => false
        }
    }

    fn handle_command(&mut self, command: Command) -> u8 {
        let hl = self.reg.hl();
        let mut branch_taken = true;

        match command {
            JR_CC_I8(cc, _) | JP_CC_U16(cc, _) | RET_CC(cc) | CALL_CC_U16(cc, _) => {
                if self.reg.cc_flag(cc) {
                    self.micro_cycle();
                }
            }
            _ => {}
        }
        match command {
            NOP => {}

            ADD_A(op) => {
                let (add, carry) = calc_with_carry(vec![self[A].value, op.unpack(&self), 0], |a, b| a.overflowing_add(b));
                self.reg.set_flags(add == 0, false, half_carry_8_add(self[A].value, op.unpack(&self), 0), carry);
                self[A].value = add;
            }
            ADC_A(op) => {
                let carry = if self.reg.flags.c { 1 } else { 0 };
                let (add, new_carry) = calc_with_carry(vec![self[A].value, op.unpack(&self), carry], |a, b| a.overflowing_add(b));
                self.reg.set_flags(add == 0, false, half_carry_8_add(self[A].value, op.unpack(&self), carry), new_carry);
                self[A].value = add;
            }
            ADC_A_HL | ADD_A_HL => {
                let carry = if let ADD_A_HL = command { 0 } else if self.reg.flags.c { 1 } else { 0 };
                let old = self.mem.read(hl);
                let (add, new_carry) = calc_with_carry(vec![self[A].value, old, carry], |a, b| a.overflowing_add(b));
                self.reg.set_flags(add == 0, false, half_carry_8_add(self[A].value, old, carry), new_carry);
                self[A].value = add;
            }
            AND_A(op) => {
                self[A].value &= op.unpack(&self);
                self.reg.set_flags(self[A].value == 0, false, true, false);
            }
            AND_A_HL => {
                self[A].value &= self.mem.read(hl);
                self.reg.set_flags(self[A].value == 0, false, true, false);
            }
            CP_A(op) => {
                let n = op.unpack(&self);
                self.reg.set_flags(self[A].value == n, true, half_carry_8_sub(self[A].value, n, 0), n > self[A].value)
            }
            CP_A_HL => {
                let n = self.mem.read(hl);
                self.reg.set_flags(self[A].value == n, true, half_carry_8_sub(self[A].value, n, 0), n > self[A].value);
            }

            DEC_R8(id) => {
                let reg = self[id].value;
                self[id].value = reg.wrapping_sub(1);
                let z = self[id].value == 0;
                self.reg.set_flags(z, true, half_carry_8_sub(reg, 1, 0), self.reg.flags.c);
            }

            INC_R8(id) => {
                let reg = self[id].value;
                self[id].value = reg.wrapping_add(1);
                let z = self[id].value == 0;
                let hc = half_carry_8_add(reg, 1, 0);
                self.reg.set_flags(z, false, hc, self.reg.flags.c);
            }
            OR_A(op) => {
                self[A].value |= op.unpack(&self);
                self.reg.set_flags(self[A].value == 0, false, false, false);
            }
            OR_A_HL => {
                self[A].value |= self.mem.read(hl);
                self.reg.set_flags(self[A].value == 0, false, false, false);
            }
            SUB_A(op) => {
                let (sub, c) = calc_with_carry(vec![self[A].value, op.unpack(&self), 0], |a, b| a.overflowing_sub(b));
                self.reg.set_flags(sub == 0, true, half_carry_8_sub(self[A].value, op.unpack(&self), 0), c);
                self[A].value = sub;
            }
            SBC_A(op) => {
                let carry = if self.reg.flags.c { 1 } else { 0 };
                let (sub, new_carry) = calc_with_carry(vec![self[A].value, op.unpack(&self), carry], |a, b| a.overflowing_sub(b));
                self.reg.set_flags(sub == 0, true, half_carry_8_sub(self[A].value, op.unpack(&self), carry), new_carry);
                self[A].value = sub;
            }
            SBC_A_HL | SUB_A_HL => {
                let carry = if let SUB_A_HL = command { 1 } else if self.reg.flags.c { 1 } else { 0 };
                let old = self.mem.read(hl);
                let (sub, new_carry) = calc_with_carry(vec![self[A].value, old, carry], |a, b| a.overflowing_sub(b));
                self.reg.set_flags(sub == 0, true, half_carry_8_sub(self[A].value, old, carry), new_carry);
                self[A].value = sub;
            }
            XOR_A(op) => {
                self[A].value ^= op.unpack(&self);
                self.reg.set_flags(self[A].value == 0, false, false, false);
            }
            XOR_A_HL => {
                self[A].value ^= self.mem.read(hl);
                self.reg.set_flags(self[A].value == 0, false, false, false);
            }
            ADD_HL_R16(reg) => {
                let hc = half_carry_16_add(hl.value(), reg.value(), 0);
                let (hl, carry) = hl.value().overflowing_add(reg.value());
                self.set_word_register_with_micro_cycle(hl, self.reg.hl());
                self.reg.set_flags(self.reg.flags.z, false, hc, carry);
            }
            DECH_HL => {
                let old = self.mem.read(hl);
                self.mem *= (hl, old.wrapping_sub(1));
                let hc = half_carry_8_sub(old, 1, 0);
                self.reg.set_flags(old.wrapping_sub(1) == 0, true, hc, self.reg.flags.c);
            }
            INCH_HL => {
                let old = self.mem.read(hl);
                self.mem *= (hl, old.wrapping_add(1));
                let hc = half_carry_8_add(old, 1, 0);
                self.reg.set_flags(old.wrapping_add(1) == 0, false, hc, self.reg.flags.c);
            }
            DEC_R16(reg) => {
                self.set_word_register_with_micro_cycle(reg.value().wrapping_sub(1), reg)
            }
            INC_R16(reg) => self.set_word_register_with_micro_cycle(reg.value().wrapping_add(1), reg),
            RR_HL | RL_HL | RRC_HL | RLC_HL | RR_R8(_) | RL_R8(_) | RLA | RRA | RLC_R8(_) | RRC_R8(_) | RLCA | RRCA => {
                let mut value = match command {
                    RL_R8(id) | RR_R8(id) | RLC_R8(id) | RRC_R8(id) => self[id].value,
                    RLA | RRA | RLCA | RRCA => self[A].value,
                    RR_HL | RL_HL | RRC_HL | RLC_HL => self.mem.read(hl),
                    _ => panic!(),
                };
                let carry = match command {
                    RLC_R8(_) | RL_R8(_) | RLA | RLCA | RLC_HL | RL_HL => value & 128 != 0,
                    _ => value & 1 != 0,
                };
                let mask_condition = match command {
                    RRC_R8(_) | RRC_HL | RRCA | RLC_R8(_) | RLC_HL | RLCA => carry,
                    _ => self.reg.flags.c
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
                    RL_R8(id) | RR_R8(id) | RLC_R8(id) | RRC_R8(id) => self[id].value = value,
                    RLA | RRA | RLCA | RRCA => self[A].value = value,
                    RR_HL | RL_HL | RRC_HL | RLC_HL => self.mem *= (hl, value),
                    _ => panic!()
                };
                self.reg.set_flags(z, false, false, carry);
            }
            SRA_R8(_) | SLA_R8(_) | SRL_R8(_) | SRL_HL | SLA_HL | SRA_HL => {
                let mut value = match command {
                    SRL_HL | SRA_HL | SLA_HL => self.mem.read(hl),
                    SLA_R8(id) | SRA_R8(id) | SRL_R8(id) => self[id].value,
                    _ => panic!(),
                };
                let carry = match command {
                    SRA_R8(_) | SRA_HL | SRL_R8(_) | SRL_HL => value & 1 != 0,
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
                    SLA_R8(id) | SRA_R8(id) | SRL_R8(id) => self[id].value = value,
                    _ => panic!()
                };
                self.reg.set_flags(value == 0, false, false, carry);
            }
            BIT_U3_R8(bit, id) => {
                self.reg.flags.z = (self[id].value & bit.0) ^ bit.0 == bit.0;
                self.reg.flags.n = false;
                self.reg.flags.h = true;
            }
            BIT_U3_HL(bit) => {
                self.reg.flags.z = (self.mem.read(hl) & bit.0) ^ bit.0 == bit.0;
                self.reg.flags.n = false;
                self.reg.flags.h = true;
            }
            RES_U3_R8(bit, id) => {
                self[id].value &= !bit.0
            }
            RES_U3_HL(bit) => {
                let x = self.mem.read(hl);
                self.mem *= (hl, x & !bit.0)
            }
            SET_U3_R8(bit, id) => self[id].value |= bit.0,
            SET_U3_HL(bit) => {
                let x = self.mem.read(hl);
                self.mem *= (hl, x | bit.0)
            }
            SWAP_R8(id) => {
                self.reg.set_flags(self[id].value == 0, false, false, false);
                self[id].value = self[id].value.rotate_left(4);
            }
            SWAP_HL => {
                let x = self.mem.read(hl);
                self.mem *= (hl, x.rotate_left(4));
                self.reg.set_flags(x == 0, false, false, false);
            }
            LD_R8_R8(a, b) => {
                self[a].value = self[b].value
            }
            LD_R8_U8(a, b) => self[a].value = b,
            LD_R16_U16(a, b) => self.set_word_register(b, a),
            LD_HL_R8(id) => { self.mem *= (hl, self[id].value); }
            LD_R8_HL(id) => { self[id].value = self.mem.read(hl) }
            LD_R16_A(n) => self.mem *= (n, self[A]),
            LDH_U16_A(n) => self.mem *= (n, self[A]),
            LDH_C_A => self.mem *= (self[C], self[A]),
            LD_A_U8(n) => {
                self[A].value = n
            },
            LD_A_R16(n) => self[A].value = self.mem.read(n),
            LDH_A_U16(n) => self[A].value = self.mem.read(n),
            LDH_A_U8(n) => {
                let x = self.mem.read(n);
                self[A].value = x;
            }
            LDH_U8_A(n) => {
                self.mem *= (n, self[A].value)
            }
            LDH_HL_U8(n) => self.mem *= (hl, n),
            LDH_A_C => self[A].value = self.mem.read(self[C]),
            LD_A_HLD => {
                self.set_word_register(hl.value().wrapping_sub(1), self.reg.hl());
                self[A].value = self.mem.read(hl);
            }
            LD_HLD_A => {
                self.set_word_register(hl.value().wrapping_sub(1), self.reg.hl());
                self.mem *= (hl, self[A]);
            }
            LD_A_HLI => {
                self[A].value = self.mem.read(hl);
                self.set_word_register(hl.value().wrapping_add(1), self.reg.hl());
            }
            LD_HLI_A => {
                self.mem *= (hl, self[A]);
                self.set_word_register(hl.value().wrapping_add(1), self.reg.hl());
            }
            CALL_U16(n) => {
                self.micro_cycle();
                let [lo, hi] = self.reg.pc.0.to_le_bytes();
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, hi);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, lo);
                self.set_pc(n, false);
            }

            JP_HL => self.set_pc(self.reg.hl().value(), false),
            JP_U16(n) => self.set_pc(n, true),
            JR_I8(n) => self.set_pc((self.reg.pc.0 as i16 + n as i16) as u16, true),
            CPL => {
                self[A].value = !self[A].value;
                self.reg.set_flags(self.reg.flags.z, true, true, self.reg.flags.c);
            }
            RET => {
                let lo = self.mem.read(self.reg.sp);
                let hi = self.mem.read(self.reg.sp.value().wrapping_add(1));
                self.set_pc(u16::from_le_bytes([lo, hi]), true);
                self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
            }
            RETI => {
                let lo = self.mem.read(self.reg.sp);
                let hi = self.mem.read(self.reg.sp.value().wrapping_add(1));
                self.set_pc(u16::from_le_bytes([lo, hi]), true);
                self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
                self.ei_counter = 1;
                self.ime = true;
            }
            RST(rst_vec) => {
                let [lo, hi] = self.reg.pc.0.to_le_bytes();
                self.set_pc(rst_vec as u16, true);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, hi);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, lo);
            }
            ADD_SP_I8(n) | LD_HL_SP_I8(n) => {
                let a = self.reg.sp.value();
                let b = n as i8 as i16 as u16;
                let h = (a & 0x000F) + (b & 0x000F) > 0x000F;
                let c = (a & 0x00FF) + (b & 0x00FF) > 0x00FF;
                self.reg.set_flags(false, false, h, c);
                if let ADD_SP_I8(_) = command { self.micro_cycle() }
                self.set_word_register_with_micro_cycle(a.wrapping_add(b), if let ADD_SP_I8(_) = command { self.reg.sp } else { self.reg.hl() })
            }
            LD_U16_SP(n) => {
                let [lo, hi] = self.reg.sp.value().to_le_bytes();
                self.mem *= (n, lo);
                self.mem *= (n + 1, hi);
            }
            LD_SP_HL => self.set_word_register_with_micro_cycle(self.reg.hl().value(), self.reg.sp),

            POP_R16(reg) => {
                match reg {
                    DoubleRegisterId::Double(ByteRegister { value: _, id: high }, ByteRegister { value: _, id: low }) => {
                        for id in &[low, high] {
                            self[*id].value = self.mem.read(self.reg.sp);
                            self.set_word_register(self.reg.sp.value().wrapping_add(1), self.reg.sp);
                        }
                    }
                    DoubleRegisterId::AccFlag(..) => {
                        self.reg.flags.set(self.mem.read(self.reg.sp));
                        self[A].value = self.mem.read(self.reg.sp.value().wrapping_add(1));
                        self.set_word_register(self.reg.sp.value().wrapping_add(2), self.reg.sp);
                    }

                    _ => panic!()
                }
            }
            PUSH_AF => {
                self.micro_cycle();
                self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                self.mem *= (self.reg.sp, self[A]);
                self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                self.mem *= (self.reg.sp, self.reg.flags.value());
            }
            PUSH_R16(reg) => {
                self.micro_cycle();
                match reg {
                    DoubleRegisterId::Double(ByteRegister { value: _, id: high }, ByteRegister { value: _, id: low }) => {
                        for id in &[high, low] {
                            self.set_word_register(self.reg.sp.value().wrapping_sub(1), self.reg.sp);
                            let value = self[*id].value;
                            self.mem *= (self.reg.sp, value);
                        }
                    }
                    _ => panic!()
                }
            }
            CCF => {
                self.reg.flags.n = false;
                self.reg.flags.h = false;
                self.reg.flags.c = !self.reg.flags.c;
            }
            DAA => {
                // note: assumes a is a uint8_t and wraps from 0xff to 0
                if !self.reg.flags.n {  // after an addition, adjust if (half-)carry occurred or if result is out of bounds
                    if self.reg.flags.c || self[A].value > 0x99 {
                        self[A].value = self[A].value.wrapping_add(0x60);
                        self.reg.flags.c = true;
                    }
                    if self.reg.flags.h || (self[A].value & 0x0f) > 0x09 {
                        self[A].value = self[A].value.wrapping_add(0x6);
                    }
                } else {
                    if self.reg.flags.c { self[A].value = self[A].value.wrapping_sub(0x60); }
                    if self.reg.flags.h { self[A].value = self[A].value.wrapping_sub(0x6); }
                }
                self.reg.flags.z = self[A].value == 0;
                self.reg.flags.h = false;
            }
            DI => self.ime = false,
            EI => {
                self.ei_counter = 2
            }
            HALT => { self.halted = true }
            SCF => {
                self.reg.flags.n = false;
                self.reg.flags.h = false;
                self.reg.flags.c = true;
            }

            RET_CC(cc) => {
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

            JP_CC_U16(cc, n) => if self.reg.cc_flag(cc) { self.set_pc(n, false) } else { branch_taken = false }

            JR_CC_I8(cc, n) => if self.reg.cc_flag(cc) { self.set_pc((self.reg.pc.0 as i16 + n as i16) as u16, false) } else { branch_taken = false }

            CALL_CC_U16(cc, n) => if self.reg.cc_flag(cc) {
                let [lo, hi] = self.reg.pc.0.to_le_bytes();
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, hi);
                self.reg.sp = StackPointer(self.reg.sp.value().wrapping_sub(1));
                self.mem *= (self.reg.sp, lo);
                self.set_pc(n, false);
            } else { branch_taken = false }

            STOP => {}
        };
        command.cycles(branch_taken)
    }

    fn micro_cycle(&mut self) {
        self.mem.micro_cycle();
    }

    fn set_pc(&mut self, value: u16, trigger_cycle: bool) {
        self.reg.pc.0 = value;
        if trigger_cycle { self.micro_cycle() }
    }

    fn set_word_register(&mut self, value: u16, reg: DoubleRegisterId) {
        self.reg.set_word_register(value, reg);
    }

    fn set_word_register_with_micro_cycle(&mut self, value: u16, reg: DoubleRegisterId) {
        self.reg.set_word_register_with_callback(value, reg, |mem| mem.micro_cycle(), &mut self.mem);
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

fn half_carry_8_add(a: u8, b: u8, c: u8) -> bool { (a & 0xF) + (b & 0xF) + c > 0xF }

fn half_carry_8_sub(a: u8, b: u8, c: u8) -> bool { (a & 0x0F) < (b & 0x0F) + c }

fn half_carry_16_add(a: u16, b: u16, c: u16) -> bool { (a & 0x07FF) + (b & 0x07FF) + c > 0x07FF }