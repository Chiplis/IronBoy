use std::cmp::{max, min};

use InterruptState::*;

use crate::Command;
use crate::gameboy::Gameboy;
use crate::instruction::Command::*;
use crate::instruction_fetcher::fetch_instruction;
use crate::interrupt::{InterruptId, InterruptState};
use crate::interrupt::InterruptId::{JoypadInt, SerialInt, StatInt, TimerInt, VBlankInt};
use crate::register::{ByteRegister, FlagRegister, RegisterId, WordRegister};
use crate::register::RegisterId::*;
use crate::register::WordRegister::StackPointer;

#[deny(unreachable_patterns)]
pub fn execute_instruction(gb: &mut Gameboy) -> u8 {
    let interrupt_cycles = if handle_interrupts(gb) { 4 } else { 0 };
    if gb.halted {
        gb.halted = interrupt_cycles == 0;
        return if gb.halted { 1 } else { 4 }
    }

    let instruction = fetch_instruction(gb);
    let (opcode, command) = (instruction.0, instruction.1);
    let line =  *gb.mem.ppu.ly();
    println!("op: {} | pc: {} | sp: {} | a: {} b: {} c: {} d: {} e: {} h: {} l: {} | f: {} | ly: {}", opcode, gb.registers.pc.0 + 1, gb.registers.sp.to_address(), gb[A].value, gb[B].value, gb[C].value, gb[D].value, gb[E].value, gb[H].value, gb[L].value, gb.registers.flags.value(), line);

    gb.registers.pc.0 += command.size() as u16;

    execute_command(gb, command, interrupt_cycles)
}

fn execute_command(gb: &mut Gameboy, command: Command, interrupt_cycles: u8) -> u8 {

    let hl = gb.registers.hl();
    let mut branch_taken = true;

    match command {
        NOP => {}

        ADD_A_R8(ByteRegister { value: n, id: _ }) | ADD_A_U8(n) => {
            let (add, carry) = calc_with_carry(vec![gb[A].value, n], &mut 0, |a, b| a.overflowing_add(b));
            gb.registers.set_flags(add == 0, false, half_carry_8_add(gb[A].value, n, 0), carry);
            gb[A].value = add;
        }
        ADC_A_R8(ByteRegister { value: n, id: _ }) | ADC_A_U8(n) => {
            let carry = if gb.registers.flags.c { 1 } else { 0 };
            let (add, new_carry) = calc_with_carry(vec![gb[A].value, n, carry], &mut 0, |a, b| a.overflowing_add(b));
            gb.registers.set_flags(add == 0, false, half_carry_8_add(gb[A].value, n, carry), new_carry);
            gb[A].value = add;
        }
        ADC_A_HL | ADD_A_HL => {
            let carry = if let ADD_A_HL = command { 1 } else { if gb.registers.flags.c { 1 } else { 0 } };
            let (add, new_carry) = calc_with_carry(vec![gb[A].value, gb.mem[hl], carry], &mut 0, |a, b| a.overflowing_add(b));
            gb.registers.set_flags(add == 0, false, half_carry_8_add(gb[A].value, gb.mem[hl], carry), new_carry);
            gb[A].value = add;
        }
        AND_A_R8(ByteRegister { value: n, id: _ }) | AND_A_U8(n) => {
            gb[A].value &= n;
            gb.registers.set_flags(gb[A].value == 0, false, true, false);
        }
        AND_A_HL => {
            gb[A].value &= gb.mem[hl];
            gb.registers.set_flags(gb[A].value == 0, false, true, false);
        }
        CP_A_R8(ByteRegister { value: n, id: _ }) | CP_A_U8(n) =>
            gb.registers.set_flags(gb[A].value == n, true, half_carry_8_sub(gb[A].value, n, 0), n > gb[A].value),
        CP_A_HL => {
            let n = gb.mem[hl];
            gb.registers.set_flags(gb[A].value == n, true, half_carry_8_sub(gb[A].value, n, 0), n > gb[A].value);
        }

        DEC_R8(ByteRegister { value: _, id: id }) => {
            let reg = gb[id].value;
            gb[id].value = reg.wrapping_sub(1);
            let z = gb[id].value == 0;
            gb.registers.set_flags(z, true, half_carry_8_sub(reg, 1, 0), gb.registers.flags.c);
        }

        INC_R8(ByteRegister { value: _, id: id }) => {
            let reg = gb[id].value;
            gb[id].value = reg.wrapping_add(1);
            let z = gb[id].value == 0;
            let hc = half_carry_8_add(reg, 1, 0);
            gb.registers.set_flags(z, false, hc, gb.registers.flags.c);
        }
        OR_A_R8(ByteRegister { value: n, id: _ }) | OR_A_U8(n) => {
            gb[A].value |= n;
            gb.registers.set_flags(gb[A].value == 0, false, false, false);
        }
        OR_A_HL => {
            gb[A].value |= gb.mem[hl];
            gb.registers.set_flags(gb[A].value == 0, false, false, false);
        }
        SUB_A_R8(ByteRegister { value: n, id: _ }) | SUB_A_U8(n) => {
            let (sub, c) = calc_with_carry(vec![gb[A].value, n], &mut 0, |a, b| a.overflowing_sub(b));
            gb[A].value = sub;
            gb.registers.set_flags(gb[A].value == 0, true, half_carry_8_sub(gb[A].value, n, 0), c);
        }
        SBC_A_R8(ByteRegister { value: n, id: _ }) | SBC_A_U8(n) => {
            let carry = if gb.registers.flags.c { 1 } else { 0 };
            let (sub, new_carry) = calc_with_carry(vec![gb[A].value, n, carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb[A].value = sub;
            gb.registers.set_flags(gb[A].value == 0, true, half_carry_8_sub(gb[A].value, n, carry), new_carry);
        }
        SBC_A_HL | SUB_A_HL => {
            let carry = if let SUB_A_HL = command { 1 } else { if gb.registers.flags.c { 1 } else { 0 } };
            let (sub, new_carry) = calc_with_carry(vec![gb[A].value, gb.mem[hl], carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb[A].value = sub;
            gb.registers.set_flags(gb[A].value == 0, true, half_carry_8_sub(gb[A].value, gb.mem[hl], carry), new_carry);
        }
        XOR_A_R8(ByteRegister { value: n, id: _ }) | XOR_A_U8(n) => {
            gb[A].value ^= n;
            gb.registers.set_flags(gb[A].value == 0, false, false, false);
        }
        XOR_A_HL => {
            gb[A].value ^= gb.mem[hl];
            gb.registers.set_flags(gb[A].value == 0, false, false, false);
        }
        ADD_HL_R16(reg) => {
            let hc = half_carry_16_add(hl.to_address(), reg.to_address(), 0);
            let (hl, carry) = hl.to_address().overflowing_add(reg.to_address());
            gb.registers.set_word_register(hl, gb.registers.hl());
            gb.registers.set_flags(gb.registers.flags.z, false, hc, carry);
        }
        DECH_HL => {
            let old = gb.mem[hl];
            gb.mem *= (hl, old.wrapping_sub(1));
            let hc = half_carry_8_sub(old, 1, 0);
            gb.registers.set_flags(gb.mem[hl] == 0, true, hc, gb.registers.flags.c);
        }
        INCH_HL => {
            let old = gb.mem[hl];
            gb.mem *= (hl, old.wrapping_add(1));
            let hc = half_carry_8_add(old, 1, 0);
            gb.registers.set_flags(gb.mem[hl] == 0, false, hc, gb.registers.flags.c);
        }
        DEC_R16(reg) => gb.registers.set_word_register(reg.to_address().wrapping_sub(1), reg),
        INC_R16(reg) => gb.registers.set_word_register(reg.to_address().wrapping_add(1), reg),
        RR_HL | RL_HL | RRC_HL | RLC_HL | RR_R8(_) | RL_R8(_) | RLA | RRA | RLC_R8(_) | RRC_R8(_) | RLCA | RRCA => {
            let mut value = match command {
                RL_R8(r) | RR_R8(r) | RLC_R8(r) | RRC_R8(r) => gb[r.id].value,
                RLA | RRA | RLCA | RRCA => gb[A].value,
                RR_HL | RL_HL | RRC_HL | RLC_HL => gb.mem[hl],
                _ => panic!(),
            };
            let carry = match command {
                RLC_R8(_) | RL_R8(_) | RLA | RLCA | RLC_HL | RL_HL => value & 128 != 0,
                _ => value & 1 != 0,
            };
            let mask_condition = match command {
                RRC_R8(_) | RRC_HL | RRCA | RLC_R8(_) | RLC_HL | RLCA => carry,
                _ => gb.registers.flags.c
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
                RL_R8(r) | RR_R8(r) | RLC_R8(r) | RRC_R8(r) => gb[r.id].value = value,
                RLA | RRA | RLCA | RRCA => gb[A].value = value,
                RR_HL | RL_HL | RRC_HL | RLC_HL => gb.mem *= (hl, value),
                _ => panic!()
            };
            gb.registers.set_flags(z, false, false, carry);
        }
        SRA_R8(_) | SLA_R8(_) | SRL_R8(_) | SRL_HL | SLA_HL | SRA_HL => {
            let mut value = match command {
                SRL_HL | SRA_HL | SLA_HL => gb.mem[hl],
                SLA_R8(r) | SRA_R8(r) => gb[r.id].value,
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
                SRL_HL | SRA_HL | SLA_HL => gb.mem *= (hl, value),
                SLA_R8(r) | SRA_R8(r) => gb[r.id].value = value,
                _ => panic!()
            };
            gb.registers.set_flags(value == 0, false, false, carry);
        }
        BIT_U3_R8(_, _) | BIT_U3_HL(_) | RES_U3_R8(_, _) |
        RES_U3_HL(_) | SET_U3_R8(_, _) | SET_U3_HL(_) => {
            match command {
                BIT_U3_R8(bit, ByteRegister { value: n, id: _ }) => gb.registers.flags.z = n & bit.0 == 0,
                BIT_U3_HL(bit) => gb.registers.flags.z = gb.mem[hl] & bit.0 == 0,
                RES_U3_R8(bit, ByteRegister { value: _, id: id }) => {
                    gb[id].value &= !bit.0
                }
                RES_U3_HL(bit) => {
                    gb.mem *= (hl, gb.mem[hl] & !bit.0)
                }
                SET_U3_R8(bit, ByteRegister { value: _, id: id }) => gb[id].value |= bit.0,
                SET_U3_HL(bit) => { gb.mem *= (hl, gb.mem[hl] | bit.0) }
                _ => panic!()
            };
        }
        SWAP_R8(ByteRegister { value: n, id: id }) => {
            gb.registers.set_flags(n == 0, false, false, false);
            gb[id].value = n.rotate_left(4);
        }
        SWAP_HL => {
            gb.registers.set_flags(hl.to_address() == 0, false, false, false);
            gb.registers.set_word_register(hl.to_address().rotate_left(8), gb.registers.hl());
        }
        LD_R8_R8(a, b) => gb[a.id].value = b.value,
        LD_R8_U8(a, b) => gb[a.id].value = b,
        LD_R16_U16(a, b) => gb.registers.set_word_register(b, a),
        LD_HL_R8(ByteRegister { value: n, id: _ }) | LD_HL_N8(n) => { gb.mem *= (hl, n); }
        LD_R8_HL(a) => {
            gb[a.id].value = gb.mem[hl]
        },
        LD_R16_A(n) => gb.mem *= (n, gb[A]),
        LDH_U16_A(n) => gb.mem *= (n, gb[A]),
        LDH_C_A => gb.mem *= (gb[C], gb[A]),
        LD_A_U8(n) => gb[A].value = n,
        LD_A_R16(n) => gb[A].value = gb.mem[n],
        LDH_A_U16(n) => gb[A].value = gb.mem[n],
        LDH_A_U8(n) => {
            let x = gb.mem[n];
            gb[A].value = x;
        }
        LDH_U8_A(n) => {
            if n == 0 {
                let v = (gb.mem[n] & 0xCF) | (gb[A].value & 0x30);
                gb.mem *= (n, v)
            } else {
                let x = gb[A].value;
                gb.mem *= (n, x);
            }
        }
        LDH_HL_U8(n) => gb.mem *= (hl, n),
        LDH_A_C => gb[A].value = gb.mem[gb[C]],
        LD_A_HLD => {
            gb.registers.set_word_register(hl.to_address().wrapping_sub(1), gb.registers.hl());
            gb[A].value = gb.mem[hl];
        }
        LD_HLD_A => {
            gb.registers.set_word_register(hl.to_address().wrapping_sub(1), gb.registers.hl());
            gb.mem *= (hl, gb[A]);
        }
        LD_A_HLI => {
            gb[A].value = gb.mem[hl];
            gb.registers.set_word_register(hl.to_address().wrapping_add(1), gb.registers.hl());
        }
        LD_HLI_A => {
            gb.mem *= (hl, gb[A]);
            gb.registers.set_word_register(hl.to_address().wrapping_add(1), gb.registers.hl());
        }
        CALL_U16(n) => {
            let [lo, hi] = gb.registers.pc.0.to_le_bytes();
            gb.registers.sp = StackPointer(gb.registers.sp.to_address() - 1);
            gb.mem *= (gb.registers.sp, hi);
            gb.registers.sp = StackPointer(gb.registers.sp.to_address() - 1);
            gb.mem *= (gb.registers.sp, lo);
            gb.registers.pc.0 = n;
        }

        JP_HL => gb.registers.pc.0 = gb.registers.hl().to_address(),
        JP_U16(n) => gb.registers.pc.0 = n,
        JR_I8(n) => gb.registers.pc.0 = (gb.registers.pc.0 as i16 + n as i16) as u16,
        CPL => {
            gb[A].value = !gb[A].value;
            gb.registers.set_flags(gb.registers.flags.z, true, true, gb.registers.flags.c);
        }
        RET => {
            let lo = gb.mem[gb.registers.sp.to_address()];
            let hi = gb.mem[gb.registers.sp.to_address().wrapping_add(1)];
            gb.registers.pc.0 = u16::from_le_bytes([lo, hi]);
            gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_add(2), gb.registers.sp);
        }
        RETI => {
            let lo = gb.mem[gb.registers.sp.to_address()];
            let hi = gb.mem[gb.registers.sp.to_address().wrapping_add(1)];
            gb.registers.pc.0 = u16::from_le_bytes([lo, hi]);
            gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_add(2), gb.registers.sp);
            gb.ime_counter = 1;
            gb.ime = true;
        }
        RST(rst_vec) => {
            let [lo, hi] = gb.registers.pc.0.to_le_bytes();
            gb.registers.sp = StackPointer(gb.registers.sp.to_address().wrapping_sub(1));
            gb.mem *= (gb.registers.sp, hi);
            gb.registers.sp = StackPointer(gb.registers.sp.to_address().wrapping_sub(1));
            gb.mem *= (gb.registers.sp, lo);
            gb.registers.pc.0 = rst_vec as u16
        }
        ADD_HL_SP => {
            let (add, carry) = gb.registers.hl().to_address().overflowing_add(gb.registers.sp.to_address());
            gb.registers.set_flags(add == 0, true, half_carry_16_add(gb.registers.hl().to_address(), gb.registers.sp.to_address(), 0), carry);
            gb.registers.set_word_register(add, gb.registers.hl());
        }
        ADD_SP_I8(n) | LD_HL_SP_I8(n) => {
            let (add, carry) = if n < 0 {
                gb.registers.sp.to_address().overflowing_sub((n as u8 & !0x80) as u16)
            } else {
                gb.registers.sp.to_address().overflowing_add((n as u8 & !0x80) as u16)
            };
            let half_carry = if n < 0 {
                half_carry_16_sub(gb.registers.sp.to_address(), (n as u8 & !0x80) as u16, 0)
            } else {
                half_carry_16_add(gb.registers.sp.to_address(), (n as u8 & !0x80) as u16, 0)
            };
            gb.registers.set_flags(false, false, half_carry, carry);
            gb.registers.set_word_register(add, if let ADD_SP_I8(n) = command { gb.registers.sp } else { gb.registers.hl() })
        }
        LD_U16_SP(n) => {
            let [lo, hi] = gb.registers.sp.to_address().to_le_bytes();
            gb.mem *= (n, lo);
            gb.mem *= (n + 1, hi);
        }
        LD_U8_A(n) => gb[A].value = n,
        LD_SP_HL => gb.registers.set_word_register(gb.registers.hl().to_address(), gb.registers.sp),

        POP_R16(reg) => {
            match reg {
                WordRegister::Double(ByteRegister { value: _, id: high }, ByteRegister { value: _, id: low }) => {
                    for id in &[low, high] {
                        gb[*id].value = gb.mem[gb.registers.sp.to_address()];
                        gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_add(1), gb.registers.sp);
                    }
                }
                WordRegister::AccFlag(mut a, mut f) => {
                    gb.registers.flags.set(gb.mem[gb.registers.sp.to_address()]);
                    gb[A].value = gb.mem[gb.registers.sp.to_address().wrapping_add(1)];
                    gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_add(2), gb.registers.sp);
                }

                _ => panic!()
            }
        }
        PUSH_AF => {
            gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_sub(1), gb.registers.sp);
            gb.mem *= (gb.registers.sp, gb[A]);
            gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_sub(1), gb.registers.sp);
            gb.mem *= (gb.registers.sp, gb.registers.flags.value());
        }
        PUSH_R16(reg) => {
            match reg {
                WordRegister::Double(ByteRegister { value: _, id: high }, ByteRegister { value: _, id: low }) => {
                    for id in &[high, low] {
                        gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_sub(1), gb.registers.sp);
                        let sp = gb.registers.sp.to_address();
                        let value = gb[*id].value;
                        gb.mem *= (gb.registers.sp, value);
                    }
                }
                _ => panic!()
            }
        }
        CCF => {
            gb.registers.flags.n = false;
            gb.registers.flags.h = false;
            gb.registers.flags.c = !gb.registers.flags.c;
        }
        DAA => {
            // note: assumes a is a uint8_t and wraps from 0xff to 0
            if !gb.registers.flags.n {  // after an addition, adjust if (half-)carry occurred or if result is out of bounds
                if gb.registers.flags.c || gb[A].value > 0x99 {
                    gb[A].value += 0x60;
                    gb.registers.flags.c = true;
                }
                if gb.registers.flags.h || (gb[A].value & 0x0f) > 0x09 {
                    gb[A].value += 0x6;
                }
            } else {
                if gb.registers.flags.c { gb[A].value -= 0x60; }
                if gb.registers.flags.h { gb[A].value -= 0x6; }
            }
            gb.registers.flags.z = gb[A].value == 0;
            gb.registers.flags.h = false;
        }
        DI => { gb.ime = false; }
        EI => { gb.ime_counter = 2 }
        HALT => gb.halted = true,
        SCF => {
            gb.registers.flags.n = false;
            gb.registers.flags.h = false;
            gb.registers.flags.c = true;
        }

        RET_CC(cc) => if gb.registers.cc_flag(cc) {
            let lo = gb.mem[gb.registers.sp.to_address()];
            let hi = gb.mem[gb.registers.sp.to_address().wrapping_add(1)];
            gb.registers.pc.0 = u16::from_le_bytes([lo, hi]);
            gb.registers.set_word_register(gb.registers.sp.to_address().wrapping_add(2), gb.registers.sp);
        } else { branch_taken = false }

        JP_CC_U16(cc, n) => if gb.registers.cc_flag(cc) { gb.registers.pc.0 = n; } else { branch_taken = false }

        JR_CC_I8(cc, n) => if gb.registers.cc_flag(cc) { gb.registers.pc.0 = (gb.registers.pc.0 as i16 + n as i16) as u16 } else { branch_taken = false }

        CALL_CC_U16(cc, n) => if gb.registers.cc_flag(cc) {
            let [lo, hi] = gb.registers.pc.0.to_le_bytes();
            gb.registers.sp = StackPointer(gb.registers.sp.to_address() - 1);
            gb.mem *= (gb.registers.sp, hi);
            gb.registers.sp = StackPointer(gb.registers.sp.to_address() - 1);
            gb.mem *= (gb.registers.sp, lo);
            gb.registers.pc.0 = n;
        } else { branch_taken = false }

        STOP => {}
    };
    command.cycles(branch_taken) + interrupt_cycles
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

fn handle_interrupts(gb: &mut Gameboy) -> bool {
    gb.ime_counter -= 1;
    if gb.ime_counter == 0 {
        gb.ime = true;
    }
    gb.ime_counter = max(gb.ime_counter, -1);
    if !gb.ime { return false; }
    for interrupt_id in &[VBlankInt, StatInt, TimerInt, SerialInt, JoypadInt] {
        if trigger_interrupt(gb, interrupt_id) {
            return true;
        }
    }
    return false;
}

fn trigger_interrupt(gb: &mut Gameboy, interrupt_id: &InterruptId) -> bool {
    let state = gb.mem.interrupt_handler.get_state(*interrupt_id);
    match state {
        Active => {
            gb.ime = false;
            gb.mem.interrupt_handler.set(vec![*interrupt_id], false);
            let [lo, hi] = gb.registers.pc.0.to_le_bytes();
            gb.registers.sp = StackPointer(gb.registers.sp.to_address() - 1);
            gb.mem *= (gb.registers.sp, hi);
            gb.registers.sp = StackPointer(gb.registers.sp.to_address() - 1);
            gb.mem *= (gb.registers.sp, lo);
            gb.registers.pc.0 = *interrupt_id as u16;
            true
        }
        Priority(priority_id) => trigger_interrupt(gb, &priority_id),
        Inactive | Enabled | Requested => false
    }
}

fn half_carry_8_add(a: u8, b: u8, c: u8) -> bool { (((a & 0xF) + ((b + c) & 0xF)) & 0x10) == 0x10 }

fn half_carry_8_sub(a: u8, b: u8, c: u8) -> bool { (((a & 0xF).wrapping_sub(b.wrapping_add(c) & 0xF)) & 0x10) == 0x10 }

fn half_carry_16_add(a: u16, b: u16, c: u16) -> bool { ((a & 0xFF).wrapping_add((b.wrapping_add(c)) & 0xFF)) & 0x10 == 0x1000 }

fn half_carry_16_sub(a: u16, b: u16, c: u16) -> bool { ((a & 0xFF).wrapping_sub(b.wrapping_add(c) & 0xFF)) & 0x10 == 0x1000 }