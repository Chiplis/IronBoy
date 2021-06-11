use crate::register::{Bit, ConditionCode, ByteRegister, SpecialRegister};
use Instruction::*;
use crate::memory_map::MemoryMap;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum Instruction {
    ADC_A_R8(ByteRegister),
    ADC_A_HL,
    ADC_A_N8(u8),
    ADD_A_R8(ByteRegister),
    ADD_A_HL,
    ADD_A_N8(u8),
    AND_A_R8(ByteRegister),
    AND_A_HL,
    AND_A_N8(u8),
    CP_A_R8(ByteRegister),
    CP_A_HL,
    CP_A_N8(u8),
    DEC_R8(ByteRegister),
    DECH_HL,
    INC_R8(ByteRegister),
    OR_A_R8(ByteRegister),
    OR_A_HL,
    OR_A_N8(u8),
    SBC_A_R8(ByteRegister),
    SBC_A_HL,
    SBC_A_N8(u8),
    SUB_A_R8(ByteRegister),
    SUB_A_HL,
    SUB_A_N8(u8),
    XOR_A_R8(ByteRegister),
    XOR_A_HL,
    XOR_A_N8(u8),
    ADD_HL_R16(SpecialRegister),
    DEC_R16(SpecialRegister),
    INC_R16(SpecialRegister),
    BIT_U3_R8(Bit, ByteRegister),
    BIT_U3_HL(Bit),
    RES_U3_R8(Bit, ByteRegister),
    RES_U3_HL(Bit),
    SET_U3_R8(Bit, ByteRegister),
    SET_U3_HL(Bit),
    SWAP_R8(ByteRegister),
    SWAP_HL,
    RL_R8(ByteRegister),
    RL_HL,
    RLA,
    RLC_R8(ByteRegister),
    RLC_HL,
    RLCA,
    RR_R8(ByteRegister),
    RR_HL,
    RRA,
    RRC_R8(ByteRegister),
    RRC_HL,
    RRCA,
    SLA_R8(ByteRegister),
    SLA_HL,
    SRA_R8(ByteRegister),
    SRA_HL,
    SRL_R8(ByteRegister),
    SRL_HL,
    LD_R8_R8(ByteRegister, ByteRegister),
    LD_R8_N8(ByteRegister, u8),
    LD_R16_N16(SpecialRegister, u16),
    LD_HL_R8(ByteRegister),
    LD_HL_N8(u8),
    LD_R8_HL(ByteRegister),
    LD_R16_A(SpecialRegister),
    LDH_N16_A(u16),
    LD_N8_A(u8),
    LDH_N8_A(u8),
    LDH_C_A,
    LD_A_R16(SpecialRegister),
    LD_A_N8(u8),
    LD_A_N16(u16),
    LDH_A_N8(u8),
    LDH_HL_N8(u8),
    LDH_A_C,
    LD_HLI_A,
    LD_HLD_A,
    LD_A_HLI,
    LD_A_HLD,
    CALL_N16(u16),
    CALL_CC_N16(ConditionCode, u16),
    JP_HL,
    JP_N16(u16),
    JP_CC_N16(ConditionCode, u16),
    JR_E8(i8),
    JR_CC_E8(ConditionCode, i8),
    RET_CC(ConditionCode),
    RET,
    RETI,
    RST(RstVec),
    ADD_HL_SP,
    ADD_SP_E8(i8),
    LD_SP_N16(u16),
    LD_N16_SP(u16),
    LD_HL_SP_E8(i8),
    LD_SP_HL,
    POP_AF,
    POP_R16(SpecialRegister),
    PUSH_AF,
    PUSH_R16(SpecialRegister),
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

impl Instruction {
    pub fn size(&self) -> u8 {
        match self {
            LD_A_N8(_) | ADC_A_N8(_) | ADD_A_N8(_) | AND_A_N8(_) | CP_A_N8(_) | OR_A_N8(_) | SBC_A_N8(_) |
            SUB_A_N8(_) | XOR_A_N8(_) | BIT_U3_R8(..) | BIT_U3_HL(..) | RES_U3_R8(..) | RES_U3_HL(..) |
            SET_U3_R8(..) | SET_U3_HL(..) | SWAP_R8(_) | SWAP_HL | RL_R8(_) | RL_HL | RLC_R8(_) |
            RLC_HL | RR_R8(_) | RR_HL | RRC_R8(_) | RRC_HL | SLA_R8(_) | SLA_HL | SRA_R8(_) |
            SRA_HL | SRL_R8(_) | SRL_HL | LD_R8_N8(..) | LD_HL_N8(..) | JR_E8(_) | JR_CC_E8(..) |
            LDH_A_N8(_) | LDH_N8_A(_) | ADD_SP_E8(_) | LD_HL_SP_E8(_) => 2,

            LDH_N16_A(_) | LD_A_N16(_) | LD_R16_N16(..) | CALL_N16(_) | CALL_CC_N16(..) |
            JP_N16(_) | JP_CC_N16(..) | LD_SP_N16(_) | LD_N16_SP(_) => 3,
            _ => 1
        }
    }

    #[deny(unreachable_patterns)]
    pub fn cycles(&self, condition: bool) -> u8 {
        match self {
            NOP => 1,
            RRCA => 1,
            STOP => 1,
            RLA => 1,
            RRA => 1,
            LD_HLI_A => 2,
            DAA => 1,
            LD_A_HLI => 2,
            CPL => 1,
            RLCA => 1,
            LD_HLD_A => 2,
            SCF => 1,
            ADD_HL_SP => 2,
            LD_A_HLD => 2,
            CCF => 1,
            HALT => 1,
            ADD_A_HL => 2,
            ADC_A_HL => 2,
            SBC_A_HL => 2,
            RETI => 4,
            RET => 4,
            DI => 1,
            PUSH_AF => 4,
            EI => 1,
            LD_SP_HL => 2,
            POP_AF => 3,
            JP_HL => 1,
            LD_R16_N16(..) => 3,
            LD_R16_A(..) => 2,
            INC_R16(..) => 2,
            INC_R8(..) => 1,
            DEC_R8(..) => 1,
            LD_R8_N8(..) => 2,
            LD_N16_SP(..) => 5,
            ADD_HL_R16(..) => 2,
            LD_A_R16(..) => 2,
            DEC_R16(..) => 2,
            ADC_A_N8(..) => 2,
            AND_A_N8(..) => 1,
            JR_E8(..) => 3,
            LDH_N16_A(..) => 4,
            LDH_N8_A(..) => 3,
            LD_SP_N16(..) => 3,

            DECH_HL => 3,
            LDH_HL_N8(..) => 3,

            LD_A_N8(..) => 2,

            LD_R8_R8(..) => 1,
            LD_R8_HL(..) => 2,
            LD_HL_R8(..) => 2,
            ADD_A_R8(..) => 1,
            ADC_A_R8(..) => 2,
            SUB_A_R8(..) => 1,
            SUB_A_HL => 2,
            LD_HL_SP_E8(..) => 3,

            SBC_A_R8(..) => 1,
            AND_A_R8(..) => 1,
            AND_A_HL => 2,
            XOR_A_R8(..) => 1,
            XOR_A_HL => 2,
            OR_A_R8(..) => 1,
            OR_A_HL => 2,
            CP_A_R8(..) => 1,
            CP_A_HL => 2,
            RET_CC(..) => 5 / 2,
            POP_R16(..) => 3,
            JP_N16(..) => 4,
            PUSH_R16(..) => 4,
            CALL_N16(..) => 6,
            SUB_A_N8(..) => 2,
            LDH_A_N8(..) => 3,
            LDH_C_A => 2,
            LDH_A_C => 2,
            SBC_A_N8(..) => 2,
            LD_N8_A(..) => 3,
            ADD_SP_E8(..) => 4,
            RST(..) => 4,
            LD_A_N16(..) => 4,
            ADD_A_N8(..) => 2,
            CP_A_N8(..) => 2,
            OR_A_N8(..) => 2,
            XOR_A_N8(..) => 2,
            LD_HL_N8(..) => 3,
            RLC_R8(..) | RRC_R8(..) | SLA_R8(..) | SWAP_R8(..) | BIT_U3_R8(..) |
            RES_U3_R8(..) | SET_U3_R8(..) | RL_R8(..) | RR_R8(..) | SRA_R8(..) |
            SRL_R8(..) => 2,
            RLC_HL | RRC_HL | SLA_HL | SWAP_HL | BIT_U3_HL(..) | SRL_HL |
            RES_U3_HL(..) | SET_U3_HL(..) | RL_HL | RR_HL | SRA_HL => 4,


            JR_CC_E8(..) => if condition { 3 } else { 2 },
            JP_CC_N16(..) => if condition { 4 } else { 3 },
            CALL_CC_N16(..) => if condition { 6 } else { 3 },

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


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InterruptId {
    VBlank = 0x40,
    STAT = 0x48,
    Timer = 0x50,
    Serial = 0x58,
    Joypad = 0x60,
}

#[derive(Copy, Clone, Debug)]
pub struct Interrupt {
    pub id: InterruptId,
    pub mask: u8,
    pub mem: [u8; 0x10000],
}