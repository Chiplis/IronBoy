use crate::instruction::InstructionOperand::{OpByte, OpHL, OpRegister};
use Command::*;

use crate::register::{Bit, ConditionCode, RegisterId, WordRegister};

pub struct Instruction(pub u8, pub Command);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstructionOperand {
    OpRegister(RegisterId),
    OpByte(u8),
    OpHL,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    AdcA(InstructionOperand),
    AddA(InstructionOperand),
    AddHlR16(WordRegister),
    AddSpI8(i8),
    AndA(InstructionOperand),
    BitU3(Bit, InstructionOperand),
    CallCcU16(ConditionCode, u16),
    CallU16(u16),
    Ccf,
    Cpl,
    CpA(InstructionOperand),
    Daa,
    DechHl,
    DecR16(WordRegister),
    DecR8(RegisterId),
    DisableInterrupt,
    EnableInterrupt,
    Halt,
    InchHl,
    IncR16(WordRegister),
    IncR8(RegisterId),
    JpCcU16(ConditionCode, u16),
    JpHl,
    JpU16(u16),
    JrCcI8(ConditionCode, i8),
    JrI8(i8),
    LdhAC,
    LdhAU16(u16),
    LdhAU8(u8),
    LdhCA,
    LdhHlU8(u8),
    LdhU16A(u16),
    LdhU8A(u8),
    LdAHld,
    LdAHli,
    LdAR16(WordRegister),
    LdAU8(u8),
    LdHldA,
    LdHliA,
    LdHlR8(RegisterId),
    LdHlSpI8(i8),
    LdR16A(WordRegister),
    LdR16U16(WordRegister, u16),
    LdR8Hl(RegisterId),
    LdR8R8(RegisterId, RegisterId),
    LdR8U8(RegisterId, u8),
    LdSpHl,
    LdU16Sp(u16),
    Nop,
    OrA(InstructionOperand),
    PopR16(WordRegister),
    PushAf,
    PushR16(WordRegister),
    ResU3Hl(Bit),
    ResU3R8(Bit, RegisterId),
    Ret,
    Reti,
    RetCc(ConditionCode),
    Rl(InstructionOperand, bool),
    Rlc(InstructionOperand, bool),
    Rr(InstructionOperand, bool),
    Rrc(InstructionOperand, bool),
    Rst(RstVec),
    SbcA(InstructionOperand),
    Scf,
    SetU3Hl(Bit),
    SetU3R8(Bit, RegisterId),
    Sla(InstructionOperand),
    Sra(InstructionOperand),
    Srl(InstructionOperand),
    Stop,
    SubA(InstructionOperand),
    SwapHl,
    SwapR8(RegisterId),
    XorA(InstructionOperand),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

#[deny(unreachable_patterns)]
impl Command {
    pub fn size(&self) -> u8 {
        match self {
            AdcA(n) | AddA(n) | AndA(n) | CpA(n) | OrA(n) | SbcA(n) | SubA(n) | XorA(n) => {
                match n {
                    OpRegister(_) | OpHL => 1,
                    OpByte(_) => 2,
                }
            }
            Rl(op, small) | Rlc(op, small) | Rr(op, small) | Rrc(op, small) => match (op, small) {
                (OpRegister(RegisterId::A), true) => 1,
                (_, false) => 2,
                (_, true) => panic!("Invalid operand/size combination for operation"),
            },
            LdAU8(..) | BitU3(..) | ResU3R8(..) | ResU3Hl(..) | SetU3R8(..) | SetU3Hl(..)
            | SwapR8(..) | SwapHl | Sla(..) | Sra(..) | Srl(..) | LdR8U8(..) | JrI8(..)
            | JrCcI8(..) | LdhAU8(..) | LdhU8A(..) | AddSpI8(..) | LdHlSpI8(..) | LdhHlU8(..) => 2,

            LdhU16A(..) | LdhAU16(..) | LdR16U16(..) | CallU16(..) | CallCcU16(..) | JpU16(..)
            | JpCcU16(..) | LdU16Sp(..) => 3,
            _ => 1,
        }
    }

    #[deny(unreachable_patterns)]
    pub fn cycles(&self, branch: bool) -> u8 {
        match self {
            AddA(n) | SubA(n) | SbcA(n) | AndA(n) | XorA(n) | OrA(n) | CpA(n) | AdcA(n) => {
                match n {
                    OpRegister(_) => 1,
                    OpByte(_) | OpHL => 2,
                }
            }

            BitU3(_, op) => match op {
                OpRegister(_) => 2,
                OpHL => 3,
                OpByte(n) => panic!("Invalid operand for BIT_U3 instruction: {}", n),
            },

            Daa | Cpl | Scf | Ccf | Halt | DisableInterrupt | EnableInterrupt | JpHl
            | IncR8(..) | DecR8(..) | LdR8R8(..) | Nop | Stop => 1,

            Sla(op) | Sra(op) | Srl(op) => match op {
                OpRegister(_) => 2,
                OpHL => 4,
                OpByte(n) => panic!("Invalid operand for BIT_U3 instruction: {}", n),
            },

            Rl(op, small) | Rlc(op, small) | Rr(op, small) | Rrc(op, small) => match (op, small) {
                (OpRegister(RegisterId::A), true) => 1,
                (OpRegister(_), false) => 2,
                (OpHL, false) => 4,
                _ => panic!("Invalid operand/size combination for operation"),
            },

            IncR16(..) | LdSpHl | LdR8U8(..) | LdHlR8(..) | LdAU8(..) | AddHlR16(..)
            | LdAR16(..) | DecR16(..) | LdhCA | LdhAC | LdR8Hl(..) | LdR16A(..) | LdAHld
            | LdAHli | LdHldA | LdHliA | SwapR8(..) | SetU3R8(..) | ResU3R8(..) => 2,

            PopR16(..) | JrI8(..) | LdhU8A(..) | DechHl | InchHl | LdhHlU8(..) | LdHlSpI8(..)
            | LdhAU8(..) | LdR16U16(..) => 3,

            LdhU16A(..) | PushAf | Reti | Ret | JpU16(..) | PushR16(..) | AddSpI8(..) | Rst(..)
            | LdhAU16(..) | SwapHl | ResU3Hl(..) | SetU3Hl(..) => 4,

            LdU16Sp(..) => 5,

            CallU16(..) => 6,

            JrCcI8(..) => {
                if branch {
                    3
                } else {
                    2
                }
            }
            JpCcU16(..) => {
                if branch {
                    4
                } else {
                    3
                }
            }
            RetCc(..) => {
                if branch {
                    5
                } else {
                    2
                }
            }
            CallCcU16(..) => {
                if branch {
                    6
                } else {
                    3
                }
            }
        }
    }
}