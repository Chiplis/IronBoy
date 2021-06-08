use crate::Instruction;
use crate::instruction_fetcher::Gameboy;
use crate::register::{ByteRegister, FlagRegister, ProgramCounter, StackPointer, RegisterId, ConditionCode};
use std::collections::HashMap;
use crate::instruction::Instruction::*;

pub fn execute_instruction(gb: &mut Gameboy, instruction: Instruction) -> &Gameboy {
    gb.pc.0 += match instruction {
        NOP => { 1 }

        ADC_A_R8(ByteRegister(n, _)) | ADC_A_HL(n) | ADC_A_N8(n) |
        ADD_A_R8(ByteRegister(n, _)) | ADD_A_HL(n) | ADD_A_N8(n) => {
            let carry = match instruction {
                ADD_A_R8(_) | ADD_A_N8(_) | ADD_A_HL(_) => 0,
                _ => if gb.f.c { 1u8 } else { 0 }
            };
            let h = half_carry_8_add(gb.a.0, n, carry);
            let (sum, c) = calc_with_carry(vec![gb.a.0, n, carry], &mut 0, |a, b| a.overflowing_add(b));
            gb.a.0 = sum;
            gb.set_flags(gb.a.0 == 0, false, h, c);
            match instruction {
                ADC_A_N8(_) | ADD_A_N8(_) => 2,
                _ => 1
            }
        }
        AND_A_R8(ByteRegister(n, _)) | AND_A_N8(n) | AND_A_HL(n) => {
            gb.a.0 &= n;
            gb.set_flags(gb.a.0 == 0, false, true, false);
            match instruction {
                ADC_A_N8(_) => 2,
                _ => 1
            }
        }
        CP_A_R8(ByteRegister(n, _)) | CP_A_N8(n) | CP_A_HL(n) => {
            gb.set_flags(gb.a.0 == n, true, half_carry_8_sub(gb.a.0, n, 0), n > gb.a.0);
            match instruction {
                CP_A_N8(_) => 2,
                _ => 1
            }
        }
        DEC_R8(ByteRegister(_, id)) => {
            let reg = gb.get_register(id).0;
            gb.get_register(id).0 = reg.wrapping_sub(1);
            let z = gb.get_register(id).0 == 0;
            gb.set_flags(z, true, half_carry_8_sub(reg, 1, 0), gb.f.c);
            1
        }
        DEC_HL(n) => {
            let r = gb.ram[n as usize];
            gb.ram[n as usize] -= 1;
            1
        }
        INC_R8(ByteRegister(_, id)) => {
            let reg = gb.get_register(id).0;
            gb.get_register(id).0 += 1;
            let z = gb.get_register(id).0 == 0;
            gb.set_flags(z, false, half_carry_8_add(reg, 1, 0), gb.f.c);
            1
        }
        INC_HL(n) => {
            let r = gb.ram[n as usize];
            gb.ram[n as usize] += 1;
            1
        }
        OR_A_R8(ByteRegister(n, _)) | OR_A_HL(n) | OR_A_N8(n) => {
            gb.a.0 |= n;
            gb.set_flags(gb.a.0 == 0, false, false, false);
            match instruction {
                OR_A_N8(_) => 2,
                _ => 1
            }
        }
        SBC_A_R8(ByteRegister(n, _)) | SBC_A_HL(n) | SBC_A_N8(n) |
        SUB_A_R8(ByteRegister(n, _)) | SUB_A_HL(n) | SUB_A_N8(n) => {
            let carry = match instruction {
                SUB_A_R8(_) | SUB_A_N8(_) | SUB_A_HL(_) => { 0 }
                _ => if gb.f.c { 1 } else { 0 }
            };
            let (sub, c) = calc_with_carry(vec![gb.a.0, n, carry], &mut 0, |a, b| a.overflowing_sub(b));
            gb.a.0 = sub;
            gb.set_flags(gb.a.0 == 0, true, c, half_carry_8_sub(gb.a.0, n, carry));
            match instruction {
                SBC_A_N8(_) | SUB_A_N8(_) => 2,
                _ => 1
            }
        }
        XOR_A_R8(ByteRegister(n, _)) | XOR_A_HL(n) | XOR_A_N8(n) => {
            gb.a.0 ^= n;
            gb.set_flags(gb.a.0 == 0, false, false, false);
            match instruction {
                XOR_A_N8(_) => 2,
                _ => 1
            }
        }
        ADD_HL_R16(reg) => {
            let hc = half_carry_16_add(gb.hl().value(), reg.value(), 0);
            let (hl, carry) = gb.hl().value().overflowing_add(reg.value());
            gb.set_word_register(hl, gb.hl());
            gb.set_flags(gb.f.z, false, hc, carry);
            1
        }
        DEC_R16(reg) => {
            gb.set_word_register(reg.value() - 1, reg);
            1
        }
        INC_R16(reg) => {
            gb.set_word_register(reg.value() + 1, reg);
            1
        }
        BIT_U3_R8(_, _) | BIT_U3_HL(_, _) | RES_U3_R8(_, _) |
        RES_U3_HL(_, _) | SET_U3_R8(_, _) | SET_U3_HL(_, _) => {
            match instruction {
                BIT_U3_R8(bit, ByteRegister(n, _)) | BIT_U3_HL(bit, n) => gb.f.z = n & bit.0 == 0,
                RES_U3_R8(bit, ByteRegister(_, id)) => gb.get_register(id).0 &= bit.0,
                RES_U3_HL(bit, n) => gb.ram[n as usize] &= bit.0,
                SET_U3_R8(bit, ByteRegister(_, id)) => gb.get_register(id).0 |= bit.0,
                SET_U3_HL(bit, n) => gb.ram[n as usize] |= bit.0,
                _ => panic!()
            };
            2
        }
        SWAP_R8(ByteRegister(n, id)) => {
            gb.set_flags(n == 0, false, false, false);
            gb.get_register(id).0 = n.rotate_left(4);
            2
        }
        SWAP_HL(n) => {
            gb.set_flags(n == 0, false, false, false);
            gb.set_word_register(n.rotate_left(8), gb.hl());
            2
        }
        RL_R8(_) | RL_HL(_) | RLA |
        RR_R8(_) | RR_HL(_) | RRA |
        RLC_R8(_) | RLC_HL(_) | RLCA |
        RRC_R8(_) | RRC_HL(_) | RRCA => {
            let value: &mut u8 = match instruction {
                RL_R8(r) | RR_R8(r) | RLC_R8(r) | RRC_R8(r) => &mut gb.get_register(r.1).0,
                RLA | RRA | RLCA | RRCA => &mut gb.a.0,
                RR_HL(n) | RL_HL(n) | RRC_HL(n) | RLC_HL(n) => &mut gb.ram[n as usize],
                _ => panic!(),
            };
            let n = *value;
            *value = match instruction {
                RLC_R8(_) | RLC_HL(_) | RLCA => value.rotate_left(1),
                RRC_R8(_) | RRC_HL(_) | RRCA => value.rotate_right(1),
                RR_R8(_) | RR_HL(_) => *value >> 1,
                RL_R8(_) | RL_HL(_) => *value << 1,
                _ => panic!()
            };
            let z = match instruction {
                RLA | RRA | RLCA | RRCA => false,
                _ => *value == 0
            };
            gb.set_flags(z, false, false, n & 128 != 0);
            match instruction {
                RLA | RRA | RLCA | RRCA => 1,
                _ => 2
            }
        }
        SRA_HL(_) | SLA_HL(_) |
        SRA_R8(_) | SLA_R8(_) |
        SRL_R8(_) | SRL_HL(_) => {
            let value: &mut u8 = match instruction {
                SRA_HL(n) | SLA_HL(n) => &mut gb.ram[n as usize],
                SLA_R8(r) | SRA_R8(r) => &mut gb.get_register(r.1).0,
                _ => panic!(),
            };
            let n = *value;
            *value = match instruction {
                SRA_R8(_) | SRA_HL(_) => ((*value as i8) >> 1) as u8,
                SRL_HL(_) | SRL_R8(_) => *value >> 1,
                SLA_R8(_) | SLA_HL(_) => ((*value as i8) << 1) as u8,
                _ => panic!()
            };
            let z = match instruction {
                RLA | RRA | RLCA | RRCA => false,
                _ => *value == 0
            };
            gb.set_flags(z, false, false, n & 128 != 0);
            2
        }
        LD_R8_R8(a, b) => {
            gb.get_register(a.1).0 = b.0;
            1
        }
        LD_R8_N8(a, b) => {
            gb.get_register(a.1).0 = b;
            2
        }
        LD_R16_N16(a, b) => {
            gb.set_word_register(b, a);
            3
        }
        LD_HL_R8(a, b) => {
            gb.ram[a as usize] = b.0;
            1
        }
        LD_HL_N8(a, b) => {
            gb.ram[a as usize] = b;
            2
        }
        LD_R8_HL(a, b) => {
            gb.get_register(a.1).0 = gb.ram[b as usize];
            1
        }
        LD_R16_A(b) => {
            gb.ram[b.value() as usize] = gb.a.0;
            1
        }
        LD_N16_A(n) => {
            gb.ram[n as usize] = gb.a.0;
            3
        }
        LDH_N8_A(n) => {
            gb.ram[0xFF00 + n as usize] = gb.a.0;
            2
        }
        LDH_C_A => {
            gb.ram[0xFF00 + gb.c.0 as usize] = gb.a.0;
            1
        }
        LD_A_R16(n) => {
            gb.a.0 = gb.ram[n.value() as usize];
            1
        }
        LD_A_N16(n) => {
            gb.a.0 = gb.ram[n as usize];
            3
        }
        LDH_A_N8(n) => {
            gb.a.0 = gb.ram[0xFF00 + n as usize];
            2
        }
        LDH_A_C => {
            gb.a.0 = gb.ram[(0xFF00 + gb.c.0 as u16) as usize];
            1
        }
        LD_HLD_A(n) => {
            gb.ram[n as usize] = gb.a.0;
            gb.set_word_register(n.wrapping_sub(1), gb.hl());
            1
        }
        LD_HLI_A(n) => {
            gb.ram[n as usize] = gb.a.0;
            gb.set_word_register(n.wrapping_add(1), gb.hl());
            1
        }
        LD_A_HLD(n) => {
            gb.a.0 = gb.ram[n as usize];
            gb.set_word_register(n.wrapping_sub(1), gb.hl());
            1
        }
        LD_A_HLI(n) => {
            gb.a.0 = gb.ram[n as usize];
            gb.set_word_register(n.wrapping_add(1), gb.hl());
            1
        }
        CALL_N16(n) => {
            gb.sp.0 -= 1;
            gb.ram[gb.sp.0 as usize] = gb.pc.0 as u8 + 3;
            gb.pc.0 = n;
            0
        }
        CALL_CC_N16(cc, n) => if gb.cc_flag(cc) {
            gb.sp.0 -= 1;
            gb.ram[gb.sp.0 as usize] = gb.pc.0 as u8 + 3;
            gb.pc.0 = n;
            0
        } else {
            3
        }
        JP_HL(n) | JP_N16(n) => {
            gb.pc.0 = n;
            0
        }
        JP_CC_N16(cc, n) => {
            if gb.cc_flag(cc) {
                gb.pc.0 = n;
                0
            } else { 3 }
        }
        JR_E8(n) => {
            gb.pc.0 = (gb.pc.0 as i16 + n as i16) as u16 + 2;
            0
        }
        JR_CC_E8(cc, n) => {
            if gb.cc_flag(cc) {
                gb.pc.0 = (gb.pc.0 as i16 + n as i16) as u16 + 2;
                0
            } else { 2 }
        }

        CPL => { gb.a.0 = !gb.a.0; gb.set_flags(gb.f.z, true, true, gb.f.c); 1 }
        _ => panic!(),
        RET_CC(cc) => {
            if gb.cc_flag(cc) {
                1
            } else {
                1
            }
        }
        RET => { 1 }
        RETI => { 1 }
        RST(rst_vec) => { 1 }
        ADD_HL_SP(n, sp) => { 1 }
        ADD_SP_E8(n) => { 2 }
        DEC_SP(sp) => { 1 }
        INC_SP(sp) => { 1 }
        LD_SP_N16(n) => { 3 }
        LD_N16_SP(n) => { 3 }
        LD_HL_SP_E8(n) => { 2 }
        LD_SP_HL(n) => { 1 }
        POP_AF => { 1 }
        POP_R16(reg) => { 1 }
        PUSH_AF => { 1 }
        PUSH_R16(reg) => { 1 }
        CCF => { 1 }
        DAA => { 1 }
        DI => { 1 }
        EI => { 1 }
        HALT => { 1 }
        SCF => { 1 }
        STOP => { 1 }
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

fn half_carry_8_add(a: u8, b: u8, c: u8) -> bool { (((a & 0xF) + ((b+c) & 0xF)) & 0x10) == 0x10 }

fn half_carry_8_sub(a: u8, b: u8, c: u8) -> bool { (((a & 0xF).wrapping_sub(b.wrapping_add(c) & 0xF)) & 0x10) == 0x10 }

fn half_carry_16_add(a: u16, b: u16, c: u16) -> bool { ((a & 0xFF).wrapping_add((b.wrapping_add(c)) & 0xFF)) & 0x10 == 0x1000 }

fn half_carry_16_sub(a: u16, b: u16, c: u16) -> bool { ((a & 0xFF).wrapping_sub(b.wrapping_add(c) & 0xFF)) & 0x10 == 0x1000 }