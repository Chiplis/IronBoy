use Command::*;

use crate::register::{Bit, ConditionCode, WordRegister, RegisterId};

pub struct Instruction(pub u8, pub Command);

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum Command {
    ADC_A_R8(RegisterId),
    ADC_A_HL,
    ADC_A_U8(u8),
    ADD_A_R8(RegisterId),
    ADD_A_HL,
    ADD_A_U8(u8),
    AND_A_R8(RegisterId),
    AND_A_HL,
    AND_A_U8(u8),
    CP_A_R8(RegisterId),
    CP_A_HL,
    CP_A_U8(u8),
    DEC_R8(RegisterId),
    DECH_HL,
    INCH_HL,
    INC_R8(RegisterId),
    OR_A_R8(RegisterId),
    OR_A_HL,
    OR_A_U8(u8),
    SBC_A_R8(RegisterId),
    SBC_A_HL,
    SBC_A_U8(u8),
    SUB_A_R8(RegisterId),
    SUB_A_HL,
    SUB_A_U8(u8),
    XOR_A_R8(RegisterId),
    XOR_A_HL,
    XOR_A_U8(u8),
    ADD_HL_R16(WordRegister),
    DEC_R16(WordRegister),
    INC_R16(WordRegister),
    BIT_U3_R8(Bit, RegisterId),
    BIT_U3_HL(Bit),
    RES_U3_R8(Bit, RegisterId),
    RES_U3_HL(Bit),
    SET_U3_R8(Bit, RegisterId),
    SET_U3_HL(Bit),
    SWAP_R8(RegisterId),
    SWAP_HL,
    RL_R8(RegisterId),
    RL_HL,
    RLA,
    RLC_R8(RegisterId),
    RLC_HL,
    RLCA,
    RR_R8(RegisterId),
    RR_HL,
    RRA,
    RRC_R8(RegisterId),
    RRC_HL,
    RRCA,
    SLA_R8(RegisterId),
    SLA_HL,
    SRA_R8(RegisterId),
    SRA_HL,
    SRL_R8(RegisterId),
    SRL_HL,
    LD_R8_R8(RegisterId, RegisterId),
    LD_R8_U8(RegisterId, u8),
    LD_R16_U16(WordRegister, u16),
    LD_HL_R8(RegisterId),
    LD_R8_HL(RegisterId),
    LD_R16_A(WordRegister),
    LDH_U16_A(u16),
    LDH_U8_A(u8),
    LDH_C_A,
    LD_A_R16(WordRegister),
    LD_A_U8(u8),
    LDH_A_U16(u16),
    LDH_A_U8(u8),
    LDH_HL_U8(u8),
    LDH_A_C,
    LD_HLI_A,
    LD_HLD_A,
    LD_A_HLI,
    LD_A_HLD,
    CALL_U16(u16),
    CALL_CC_U16(ConditionCode, u16),
    JP_HL,
    JP_U16(u16),
    JP_CC_U16(ConditionCode, u16),
    JR_I8(i8),
    JR_CC_I8(ConditionCode, i8),
    RET_CC(ConditionCode),
    RET,
    RETI,
    RST(RstVec),
    ADD_SP_I8(i8),
    LD_U16_SP(u16),
    LD_HL_SP_I8(i8),
    LD_SP_HL,
    POP_R16(WordRegister),
    PUSH_AF,
    PUSH_R16(WordRegister),
    CCF,
    CPL,
    DAA,
    DI,
    EI,
    HALT,
    NOP,
    SCF,
    STOP,
}
#[deny(unreachable_patterns)]
impl Command {
    pub fn size(&self) -> u8 {
        match self {
            LD_A_U8(_) | ADC_A_U8(_) | ADD_A_U8(_) | AND_A_U8(_) | CP_A_U8(_) | OR_A_U8(_) | SBC_A_U8(_) |
            SUB_A_U8(_) | XOR_A_U8(_) | BIT_U3_R8(..) | BIT_U3_HL(..) | RES_U3_R8(..) | RES_U3_HL(..) |
            SET_U3_R8(..) | SET_U3_HL(..) | SWAP_R8(_) | SWAP_HL | RL_R8(_) | RL_HL | RLC_R8(_) |
            RLC_HL | RR_R8(_) | RR_HL | RRC_R8(_) | RRC_HL | SLA_R8(_) | SLA_HL | SRA_R8(_) |
            SRA_HL | SRL_R8(_) | SRL_HL | LD_R8_U8(..) | JR_I8(_) | JR_CC_I8(..) |
            LDH_A_U8(_) | LDH_U8_A(_) | ADD_SP_I8(_) | LD_HL_SP_I8(_) | LDH_HL_U8(..) => 2,

            LDH_U16_A(_) | LDH_A_U16(_) | LD_R16_U16(..) | CALL_U16(_) | CALL_CC_U16(..) |
            JP_U16(_) | JP_CC_U16(..) | LD_U16_SP(_) => 3,
            _ => 1
        }
    }

    #[deny(unreachable_patterns)]
    pub fn cycles(&self, branch: bool) -> u8 {
        match self {
            DAA | CPL | RLCA | SCF | CCF | HALT | DI | EI | JP_HL | INC_R8(..) |
            DEC_R8(..) | LD_R8_R8(..) | ADD_A_R8(..) | SUB_A_R8(..) |
            SBC_A_R8(..) | AND_A_R8(..) | XOR_A_R8(..) | OR_A_R8(..) | CP_A_R8(..) |
            ADC_A_R8(..) | NOP | RRCA | STOP | RLA | RRA => 1,

            CP_A_HL | INC_R16(..) | LD_SP_HL | LD_R8_U8(..) | XOR_A_HL | AND_A_HL | LD_HL_R8(..) |
            LD_A_U8(..) | ADD_HL_R16(..) | LD_A_R16(..) | DEC_R16(..) | ADC_A_U8(..) |
            SUB_A_U8(..) | OR_A_HL | LDH_C_A | LDH_A_C | SBC_A_U8(..) | ADD_A_U8(..) | CP_A_U8(..) |
            SRL_R8(..) | OR_A_U8(..) | XOR_A_U8(..) | LD_R8_HL(..) | SUB_A_HL | LD_R16_A(..) |
            ADD_A_HL | ADC_A_HL | SBC_A_HL | LD_A_HLD | LD_A_HLI | LD_HLD_A | LD_HLI_A |
            RLC_R8(..) | RL_R8(..) | SLA_R8(..) | SWAP_R8(..) | BIT_U3_R8(..) | SET_U3_R8(..) | RES_U3_R8(..) |
            RR_R8(..) | SRA_R8(..) | RRC_R8(..) | AND_A_U8(..) => 2,

            POP_R16(..) | JR_I8(..) | LDH_U8_A(..) | BIT_U3_HL(..) | DECH_HL | INCH_HL |
            LDH_HL_U8(..) | LD_HL_SP_I8(..) | LDH_A_U8(..) | LD_R16_U16(..) => 3,

            LDH_U16_A(..) | PUSH_AF | RETI | RET | JP_U16(..) | PUSH_R16(..) |
            ADD_SP_I8(..) | RST(..) | LDH_A_U16(..) | RLC_HL | RRC_HL | SLA_HL | SWAP_HL |
            SRL_HL | RES_U3_HL(..) | SET_U3_HL(..) | RL_HL | RR_HL | SRA_HL => 4,

            LD_U16_SP(..) => 5,

            CALL_U16(..) => 6,

            JR_CC_I8(..) => if branch { 3 } else { 2 },
            JP_CC_U16(..) => if branch { 4 } else { 3 },
            RET_CC(..) => if branch { 5 } else { 2 },
            CALL_CC_U16(..) => if branch { 6 } else { 3 },
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum RstVec {
    X00 = 0x00,
    X08 = 0x08,
    X10 = 0x10,
    X18 = 0x18,
    X20 = 0x20,
    X28 = 0x28,
    X30 = 0x30,
    X38 = 0x38,
}