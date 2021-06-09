use crate::Instruction;
use crate::instruction_fetcher::Gameboy;
use crate::register::{ByteRegister};
use crate::instruction::Instruction::*;
use crate::register::SpecialRegister::StackPointer;

pub fn execute_instruction(gb: &mut Gameboy, instruction: Instruction) -> &Gameboy {
    gb.pc.0 += instruction.size() as u16;
    match instruction {
        NOP => {}

        ADD_A_R8(ByteRegister(n, _)) | ADD_A_N8(n) => {
            let (add, carry) = calc_with_carry(vec![gb.a.0, n], &mut 0, |a, b| a.overflowing_sub(b));
            gb.set_flags(add == 0, true, half_carry_8_add(gb.a.0, n, 0), carry);
            gb.a.0 = add;
        }
        ADC_A_R8(ByteRegister(n, _)) | ADC_A_N8(n) => {
            let carry = if gb.f.c { 1 } else { 0 };
            let (add, new_carry) = calc_with_carry(vec![gb.a.0, n, carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb.set_flags(add == 0, true, half_carry_8_add(gb.a.0, n, carry), new_carry);
            gb.a.0 = add;
        }
        ADC_A_HL | ADD_A_HL => {
            let carry = if let ADD_A_HL = instruction { 1 } else { if gb.f.c { 1 } else { 0 } };
            let (add, new_carry) = calc_with_carry(vec![gb.a.0, gb.mem[gb.hl()], carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb.set_flags(add == 0, true, half_carry_8_add(gb.a.0, gb.mem[gb.hl()], carry), new_carry);
            gb.a.0 = add;
        }
        AND_A_R8(ByteRegister(n, _)) | AND_A_N8(n) => {
            gb.a.0 &= n;
            gb.set_flags(gb.a.0 == 0, false, true, false);
        }
        AND_A_HL => {
            gb.a.0 &= gb.mem[gb.hl()];
            gb.set_flags(gb.a.0 == 0, false, true, false);
        }
        CP_A_R8(ByteRegister(n, _)) | CP_A_N8(n) =>
            gb.set_flags(gb.a.0 == n, true, half_carry_8_sub(gb.a.0, n, 0), n > gb.a.0),
        CP_A_HL => {
            let n = gb.mem[gb.hl()];
            gb.set_flags(gb.a.0 == n, true, half_carry_8_sub(gb.a.0, n, 0), n > gb.a.0);
        }

        DEC_R8(ByteRegister(_, id)) => {
            let reg = gb.get_register(id).0;
            gb.get_register(id).0 = reg.wrapping_sub(1);
            let z = gb.get_register(id).0 == 0;
            gb.set_flags(z, true, half_carry_8_sub(reg, 1, 0), gb.f.c);
        }
        DEC_HL => gb.set_word_register(gb.hl().value().wrapping_sub(1), gb.hl()),
        INC_R8(ByteRegister(_, id)) => {
            let reg = gb.get_register(id).0;
            gb.get_register(id).0 += 1;
            let z = gb.get_register(id).0 == 0;
            gb.set_flags(z, false, half_carry_8_add(reg, 1, 0), gb.f.c);
        }
        INC_HL => gb.set_word_register(gb.hl().value().wrapping_add(1), gb.hl()),
        OR_A_R8(ByteRegister(n, _)) | OR_A_N8(n) => {
            gb.a.0 |= n;
            gb.set_flags(gb.a.0 == 0, false, false, false);
        }
        OR_A_HL => {
            gb.a.0 |= gb.mem[gb.hl()];
            gb.set_flags(gb.a.0 == 0, false, false, false);
        }
        SUB_A_R8(ByteRegister(n, _)) | SUB_A_N8(n) => {
            let (sub, c) = calc_with_carry(vec![gb.a.0, n], &mut 0, |a, b| a.overflowing_sub(b));
            gb.a.0 = sub;
            gb.set_flags(gb.a.0 == 0, true, half_carry_8_sub(gb.a.0, n, 0), c);
        }
        SBC_A_R8(ByteRegister(n, _)) | SBC_A_N8(n) => {
            let carry = if gb.f.c { 1 } else { 0 };
            let (sub, new_carry) = calc_with_carry(vec![gb.a.0, n, carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb.a.0 = sub;
            gb.set_flags(gb.a.0 == 0, true, half_carry_8_sub(gb.a.0, n, carry), new_carry);
        }
        SBC_A_HL | SUB_A_HL => {
            let carry = if let SUB_A_HL = instruction { 1 } else { if gb.f.c { 1 } else { 0 } };
            let (sub, new_carry) = calc_with_carry(vec![gb.a.0, gb.mem[gb.hl()], carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb.a.0 = sub;
            gb.set_flags(gb.a.0 == 0, true, half_carry_8_sub(gb.a.0, gb.mem[gb.hl()], carry), new_carry);
        }
        XOR_A_R8(ByteRegister(n, _)) | XOR_A_N8(n) => {
            gb.a.0 ^= n;
            gb.set_flags(gb.a.0 == 0, false, false, false);
        }
        XOR_A_HL => {
            gb.a.0 ^= gb.mem[gb.hl()];
            gb.set_flags(gb.a.0 == 0, false, false, false);
        }
        ADD_HL_R16(reg) => {
            let hc = half_carry_16_add(gb.hl().value(), reg.value(), 0);
            let (hl, carry) = gb.hl().value().overflowing_add(reg.value());
            gb.set_word_register(hl, gb.hl());
            gb.set_flags(gb.f.z, false, hc, carry);
        }
        DEC_R16(reg) => gb.set_word_register(reg.value() - 1, reg),
        INC_R16(reg) => gb.set_word_register(reg.value() + 1, reg),
        BIT_U3_R8(_, _) | BIT_U3_HL(_) | RES_U3_R8(_, _) |
        RES_U3_HL(_) | SET_U3_R8(_, _) | SET_U3_HL(_) => {
            match instruction {
                BIT_U3_R8(bit, ByteRegister(n, _)) => gb.f.z = n & bit.0 == 0,
                BIT_U3_HL(bit) => gb.f.z = gb.mem[gb.hl()] & bit.0 == 0,
                RES_U3_R8(bit, ByteRegister(_, id)) => gb.get_register(id).0 &= bit.0,
                RES_U3_HL(bit) => {
                    let hl = gb.hl();
                    gb.mem[hl] &= bit.0
                }
                SET_U3_R8(bit, ByteRegister(_, id)) => gb.get_register(id).0 |= bit.0,
                SET_U3_HL(bit) => {
                    let hl = gb.hl();
                    gb.mem[hl] |= bit.0
                },
                _ => panic!()
            };
        }
        SWAP_R8(ByteRegister(n, id)) => {
            gb.set_flags(n == 0, false, false, false);
            gb.get_register(id).0 = n.rotate_left(4);
        }
        SWAP_HL => {
            gb.set_flags(gb.hl().value() == 0, false, false, false);
            gb.set_word_register(gb.hl().value().rotate_left(8), gb.hl());
        }
        RL_R8(_) | RL_HL | RLA |
        RR_R8(_) | RR_HL | RRA |
        RLC_R8(_) | RLC_HL | RLCA |
        RRC_R8(_) | RRC_HL | RRCA => {
            let value: &mut u8 = match instruction {
                RL_R8(r) | RR_R8(r) | RLC_R8(r) | RRC_R8(r) => &mut gb.get_register(r.1).0,
                RLA | RRA | RLCA | RRCA => &mut gb.a.0,
                RR_HL | RL_HL | RRC_HL | RLC_HL => {
                    let hl = gb.hl();
                    &mut gb.mem[hl]
                },
                _ => panic!(),
            };
            let n = *value;
            *value = match instruction {
                RLC_R8(_) | RLC_HL | RLCA => value.rotate_left(1),
                RRC_R8(_) | RRC_HL | RRCA => value.rotate_right(1),
                RR_R8(_) | RR_HL => *value >> 1,
                RL_R8(_) | RL_HL => *value << 1,
                _ => panic!()
            };
            let z = match instruction {
                RLA | RRA | RLCA | RRCA => false,
                _ => *value == 0
            };
            gb.set_flags(z, false, false, n & 128 != 0);
        }
        SRA_HL | SLA_HL |
        SRA_R8(_) | SLA_R8(_) |
        SRL_R8(_) | SRL_HL => {
            let value: &mut u8 = match instruction {
                SRA_HL | SLA_HL => {
                    let hl = gb.hl();
                    &mut gb.mem[hl]
                },
                SLA_R8(r) | SRA_R8(r) => &mut gb.get_register(r.1).0,
                _ => panic!(),
            };
            let n = *value;
            *value = match instruction {
                SRA_R8(_) | SRA_HL => ((*value as i8) >> 1) as u8,
                SRL_HL | SRL_R8(_) => *value >> 1,
                SLA_R8(_) | SLA_HL => ((*value as i8) << 1) as u8,
                _ => panic!()
            };
            let z = match instruction {
                RLA | RRA | RLCA | RRCA => false,
                _ => *value == 0
            };
            gb.set_flags(z, false, false, n & 128 != 0);
        }
        LD_R8_R8(a, b) => gb.get_register(a.1).0 = b.0,
        LD_R8_N8(a, b) => gb.get_register(a.1).0 = b,
        LD_R16_N16(a, b) => gb.set_word_register(b, a),
        LD_HL_R8(ByteRegister(n, _)) | LD_HL_N8(n) => gb.mem.write_byte(gb.hl().value(), n),
        LD_R8_HL(a) => gb.get_register(a.1).0 = gb.mem[gb.hl()],
        LD_R16_A(n) => gb.mem.write_byte(n.value(), gb.a.0),
        LDH_N16_A(n) => gb.mem.write_byte(n, gb.a.0),
        LDH_N8_A(n) => gb.mem.write_offset(n, gb.a.0),
        LDH_C_A => gb.mem.write_register(gb.c, gb.a.0),
        LD_A_N8(n) => gb.a.0 = n,
        LD_A_R16(n) => gb.a.0 = gb.mem[n],
        LD_A_N16(n) => gb.a.0 = gb.mem[n],
        LDH_A_N8(n) => gb.a.0 = gb.mem[n],
        LDH_A_C => gb.a.0 = gb.mem[gb.c],
        LD_HLD_A => {
            gb.mem.write_byte(gb.hl().value(), gb.a.0);
            gb.set_word_register(gb.hl().value().wrapping_sub(1), gb.hl());
        }
        LD_HLI_A => {
            gb.mem.write_byte(gb.hl().value(), gb.a.0);
            gb.set_word_register(gb.hl().value().wrapping_add(1), gb.hl());
        }
        LD_A_HLD => {
            gb.a.0 = gb.mem[gb.hl()];
            gb.set_word_register(gb.hl().value().wrapping_sub(1), gb.hl());
        }
        LD_A_HLI => {
            gb.a.0 = gb.mem[gb.hl()];
            gb.set_word_register(gb.hl().value().wrapping_add(1), gb.hl());
        }
        CALL_N16(n) => {
            gb.sp = StackPointer(gb.sp.value() - 1);
            let [lo, hi] = gb.pc.0.to_le_bytes();
            gb.mem[gb.sp] = hi;
            gb.sp = StackPointer(gb.sp.value() - 1);
            gb.mem[gb.sp] = lo;
            gb.pc.0 = n;
        }
        CALL_CC_N16(cc, n) => if gb.cc_flag(cc) {
            gb.sp = StackPointer(gb.sp.value() - 1);
            let [lo, hi] = gb.pc.0.to_le_bytes();
            gb.mem[gb.sp] = hi;
            gb.sp = StackPointer(gb.sp.value() - 1);
            gb.mem[gb.sp] = lo;
            gb.pc.0 = n;
        }
        JP_HL => gb.pc.0 = gb.hl().value(),
        JP_N16(n) => gb.pc.0 = n,
        JP_CC_N16(cc, n) => if gb.cc_flag(cc) { gb.pc.0 = n }
        JR_E8(n) => gb.pc.0 = (gb.pc.0 as i16 + n as i16) as u16,
        JR_CC_E8(cc, n) => if gb.cc_flag(cc) { gb.pc.0 = (gb.pc.0 as i16 + n as i16) as u16 }
        CPL => {
            gb.a.0 = !gb.a.0;
            gb.set_flags(gb.f.z, true, true, gb.f.c);
        }
        _ => panic!(),
        RET_CC(cc) => { if gb.cc_flag(cc) {} else {} }
        RET => {}
        RETI => {}
        RST(rst_vec) => {}
        ADD_HL_SP => {}
        ADD_SP_E8(n) => {}
        DEC_SP(sp) => {}
        INC_SP(sp) => {}
        LD_SP_N16(n) => {}
        LD_N16_SP(n) => {}
        LD_HL_SP_E8(n) => {}
        LD_SP_HL => {}
        POP_AF => {}
        POP_R16(reg) => {}
        PUSH_AF => {}
        PUSH_R16(reg) => {}
        CCF => {}
        DAA => {}
        DI => {}
        EI => {}
        HALT => {}
        SCF => {}
        STOP => {}
    };

    return gb;
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