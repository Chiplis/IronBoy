use Command::*;
use crate::instruction::InstructionOperand::{OpByte, OpRegister, OpHL};

use crate::register::{Bit, ConditionCode, DoubleRegisterId, RegisterId};

pub struct Instruction(pub u8, pub Command);

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum InstructionOperand {
    OpRegister(RegisterId),
    OpByte(u8),
    OpHL
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Command {
    ADC_A(InstructionOperand),
    ADD_A(InstructionOperand),
    ADD_HL_R16(DoubleRegisterId),
    ADD_SP_I8(i8),
    AND_A(InstructionOperand),
    BIT_U3(Bit, InstructionOperand),
    CALL_CC_U16(ConditionCode, u16),
    CALL_U16(u16),
    CCF,
    CPL,
    CP_A(InstructionOperand),
    DAA,
    DECH_HL,
    DEC_R16(DoubleRegisterId),
    DEC_R8(RegisterId),
    DI,
    EI,
    HALT,
    INCH_HL,
    INC_R16(DoubleRegisterId),
    INC_R8(RegisterId),
    JP_CC_U16(ConditionCode, u16),
    JP_HL,
    JP_U16(u16),
    JR_CC_I8(ConditionCode, i8),
    JR_I8(i8),
    LDH_A_C,
    LDH_A_U16(u16),
    LDH_A_U8(u8),
    LDH_C_A,
    LDH_HL_U8(u8),
    LDH_U16_A(u16),
    LDH_U8_A(u8),
    LD_A_HLD,
    LD_A_HLI,
    LD_A_R16(DoubleRegisterId),
    LD_A_U8(u8),
    LD_HLD_A,
    LD_HLI_A,
    LD_HL_R8(RegisterId),
    LD_HL_SP_I8(i8),
    LD_R16_A(DoubleRegisterId),
    LD_R16_U16(DoubleRegisterId, u16),
    LD_R8_HL(RegisterId),
    LD_R8_R8(RegisterId, RegisterId),
    LD_R8_U8(RegisterId, u8),
    LD_SP_HL,
    LD_U16_SP(u16),
    NOP,
    OR_A(InstructionOperand),
    POP_R16(DoubleRegisterId),
    PUSH_AF,
    PUSH_R16(DoubleRegisterId),
    RES_U3_HL(Bit),
    RES_U3_R8(Bit, RegisterId),
    RET,
    RETI,
    RET_CC(ConditionCode),
    RLA,
    RLCA,
    RLC_HL,
    RLC_R8(RegisterId),
    RL_HL,
    RL_R8(RegisterId),
    RRA,
    RRCA,
    RRC_HL,
    RRC_R8(RegisterId),
    RR_HL,
    RR_R8(RegisterId),
    RST(RstVec),
    SBC_A(InstructionOperand),
    SCF,
    SET_U3_HL(Bit),
    SET_U3_R8(Bit, RegisterId),
    SLA(InstructionOperand),
    SRA(InstructionOperand),
    SRL(InstructionOperand),
    STOP,
    SUB_A(InstructionOperand),
    SWAP_HL,
    SWAP_R8(RegisterId),
    XOR_A(InstructionOperand)
}

#[deny(unreachable_patterns)]
impl Command {
    pub fn size(&self) -> u8 {
        match self {
            ADC_A(n) | ADD_A(n) | AND_A(n) |
            CP_A(n) | OR_A(n) | SBC_A(n) |
            SUB_A(n) | XOR_A(n) => match n {
                OpRegister(_) | OpHL => 1,
                OpByte(_) => 2
            }
            LD_A_U8(..) | BIT_U3(..) | RES_U3_R8(..) | RES_U3_HL(..) |
            SET_U3_R8(..) | SET_U3_HL(..) | SWAP_R8(..) | SWAP_HL | RL_R8(..) | RL_HL | RLC_R8(..) |
            RLC_HL | RR_R8(..) | RR_HL | RRC_R8(..) | RRC_HL | SLA(..) | SRA(..) | SRL(..) | LD_R8_U8(..) | JR_I8(..) | JR_CC_I8(..) |
            LDH_A_U8(..) | LDH_U8_A(..) | ADD_SP_I8(..) | LD_HL_SP_I8(..) | LDH_HL_U8(..) => 2,

            LDH_U16_A(..) | LDH_A_U16(..) | LD_R16_U16(..) | CALL_U16(..) | CALL_CC_U16(..) |
            JP_U16(..) | JP_CC_U16(..) | LD_U16_SP(..) => 3,
            _ => 1
        }
    }

    #[deny(unreachable_patterns)]
    pub fn cycles(&self, branch: bool) -> u8 {
        match self {
            ADD_A(n) | SUB_A(n) |
            SBC_A(n) | AND_A(n) | XOR_A(n) | OR_A(n) |
            CP_A(n) | ADC_A(n) => match n {
                OpRegister(_) => 1,
                OpByte(_) | OpHL => 2
            }

            BIT_U3(_, op) => match op {
                OpRegister(_) => 2,
                OpHL => 3,
                OpByte(n) => panic!("Invalid operand for BIT_U3 instruction: {}", n)
            }

            DAA | CPL | RLCA | SCF | CCF | HALT | DI | EI | JP_HL | INC_R8(..) |
            DEC_R8(..) | LD_R8_R8(..) | NOP | RRCA | STOP | RLA | RRA => 1,

            SLA(op) | SRA(op) | SRL(op) => match op {
                OpRegister(_) => 2,
                OpHL => 4,
                OpByte(n) => panic!("Invalid operand for BIT_U3 instruction: {}", n)
            }

            INC_R16(..) | LD_SP_HL | LD_R8_U8(..) | LD_HL_R8(..) |
            LD_A_U8(..) | ADD_HL_R16(..) | LD_A_R16(..) | DEC_R16(..) |
            LDH_C_A | LDH_A_C | LD_R8_HL(..) | LD_R16_A(..) |
            LD_A_HLD | LD_A_HLI | LD_HLD_A | LD_HLI_A | RLC_R8(..) | RL_R8(..) |
            SWAP_R8(..) | SET_U3_R8(..) | RES_U3_R8(..) | RR_R8(..) | RRC_R8(..) => 2,

            POP_R16(..) | JR_I8(..) | LDH_U8_A(..) |
            DECH_HL | INCH_HL | LDH_HL_U8(..) | LD_HL_SP_I8(..) | LDH_A_U8(..) | LD_R16_U16(..) => 3,

            LDH_U16_A(..) | PUSH_AF | RETI | RET | JP_U16(..) | PUSH_R16(..) |
            ADD_SP_I8(..) | RST(..) | LDH_A_U16(..) | RLC_HL | RRC_HL  | SWAP_HL |
            RES_U3_HL(..) | SET_U3_HL(..) | RL_HL | RR_HL => 4,

            LD_U16_SP(..) => 5,

            CALL_U16(..) => 6,

            JR_CC_I8(..) => if branch { 3 } else { 2 },
            JP_CC_U16(..) => if branch { 4 } else { 3 },
            RET_CC(..) => if branch { 5 } else { 2 },
            CALL_CC_U16(..) => if branch { 6 } else { 3 },
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
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