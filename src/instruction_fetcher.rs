use std::cmp::max;
use RegisterOperand::Operand;

use crate::instruction::Command::*;
use crate::instruction::Operand::{OpByte, OpHL, OpRegister};
use crate::instruction::{Instruction, RstVec};
use crate::instruction_fetcher::RegisterOperand::HL;
use crate::mmu::MemoryManagementUnit;
use crate::register::RegisterId::*;
use crate::register::{Bit, ConditionCode, Register, RegisterId};

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
enum RegisterOperand {
    HL,
    Operand(RegisterId),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct Fetcher;

impl Fetcher {
    pub fn fetch(
        halt_bug: bool,
        pc: u16,
        reg: &Register,
        ram: &mut MemoryManagementUnit,
    ) -> Instruction {
        let opcode = ram.read(pc);
        let register_ids = [B, C, D, E, H, L, A];
        let operands = [
            Operand(B),
            Operand(C),
            Operand(D),
            Operand(E),
            Operand(H),
            Operand(L),
            HL,
            Operand(A),
        ];
        let operand_idx = ((opcode & 0x0F) % 8) as usize;
        let register_idx = (max(0x40, opcode) as usize - 0x40) / 8;

        let pc_offset = u16::from(!halt_bug);
        let pc = [pc, pc + pc_offset, pc + pc_offset + 1];

        Instruction(
            opcode,
            match opcode {
                0xCB => {
                    let cb_opcode = ram.read(pc[1]) as u8;

                    let bit: usize =
                        ((cb_opcode as usize % 0x40) >> 4) * 2 + usize::from(cb_opcode & 0x0F > 7);
                    if bit > 7 {
                        panic!("Bit parsing is failing: {}.", bit)
                    };

                    let mask = [1, 2, 4, 8, 16, 32, 64, 128];
                    let bit_idx = ((cb_opcode & 0x0F) % 8) as usize;

                    match cb_opcode {
                        0x00..=0x07 => match operands[bit_idx] {
                            RegisterOperand::HL => Rlc(OpHL, false),
                            Operand(id) => Rlc(OpRegister(id), false),
                        },

                        0x08..=0x0F => match operands[bit_idx] {
                            RegisterOperand::HL => Rrc(OpHL, false),
                            Operand(id) => Rrc(OpRegister(id), false),
                        },

                        0x10..=0x17 => match operands[bit_idx] {
                            RegisterOperand::HL => Rl(OpHL, false),
                            Operand(id) => Rl(OpRegister(id), false),
                        },

                        0x18..=0x1F => match operands[bit_idx] {
                            RegisterOperand::HL => Rr(OpHL, false),
                            Operand(id) => Rr(OpRegister(id), false),
                        },

                        0x20..=0x27 => match operands[bit_idx] {
                            RegisterOperand::HL => Sla(OpHL),
                            Operand(id) => Sla(OpRegister(id)),
                        },

                        0x28..=0x2F => match operands[bit_idx] {
                            RegisterOperand::HL => Sra(OpHL),
                            Operand(id) => Sra(OpRegister(id)),
                        },

                        0x30..=0x37 => match operands[bit_idx] {
                            RegisterOperand::HL => SwapHl,
                            Operand(id) => SwapR8(id),
                        },

                        0x38..=0x3F => match operands[bit_idx] {
                            RegisterOperand::HL => Srl(OpHL),
                            Operand(id) => Srl(OpRegister(id)),
                        },
                        0x40..=0x7F => match operands[bit_idx] {
                            RegisterOperand::HL => BitU3(Bit(mask[bit]), OpHL),
                            Operand(id) => BitU3(Bit(mask[bit]), OpRegister(id)),
                        },

                        0x80..=0xBF => match operands[bit_idx] {
                            RegisterOperand::HL => ResU3Hl(Bit(mask[bit])),
                            Operand(id) => ResU3R8(Bit(mask[bit]), id),
                        },

                        0xC0..=0xFF => match operands[bit_idx] {
                            RegisterOperand::HL => SetU3Hl(Bit(mask[bit])),
                            Operand(id) => SetU3R8(Bit(mask[bit]), id),
                        },
                    }
                }

                0x06 => LdR8U8(B, ram.read(pc[1])),
                0x0E => LdR8U8(C, ram.read(pc[1])),
                0x16 => LdR8U8(D, ram.read(pc[1])),
                0x1E => LdR8U8(E, ram.read(pc[1])),
                0x26 => LdR8U8(H, ram.read(pc[1])),
                0x2E => LdR8U8(L, ram.read(pc[1])),

                0x40..=0x6F => match operands[operand_idx] {
                    RegisterOperand::HL => LdR8Hl(register_ids[register_idx]),
                    Operand(id) => LdR8R8(register_ids[register_idx], id),
                },

                0x70..=0x75 => match operands[operand_idx] {
                    Operand(id) => LdHlR8(id),
                    RegisterOperand::HL => panic!(),
                },

                0x78..=0x7D => LdR8R8(A, register_ids[opcode as usize - 0x78]),

                0x77 => LdHlR8(A),
                0x7E => LdR8Hl(A),
                0x7F => LdR8R8(A, A),

                0x80..=0x87 => match operands[operand_idx] {
                    RegisterOperand::HL => AddA(OpHL),
                    Operand(id) => AddA(OpRegister(id)),
                },

                0x88..=0x8F => match operands[operand_idx] {
                    RegisterOperand::HL => AdcA(OpHL),
                    Operand(id) => AdcA(OpRegister(id)),
                },

                0x90..=0x97 => match operands[operand_idx] {
                    RegisterOperand::HL => SubA(OpHL),
                    Operand(id) => SubA(OpRegister(id)),
                },

                0x98..=0x9F => match operands[operand_idx] {
                    RegisterOperand::HL => SbcA(OpHL),
                    Operand(id) => SbcA(OpRegister(id)),
                },

                0xA0..=0xA7 => match operands[operand_idx] {
                    RegisterOperand::HL => AndA(OpHL),
                    Operand(id) => AndA(OpRegister(id)),
                },

                0xA8..=0xAF => match operands[operand_idx] {
                    RegisterOperand::HL => XorA(OpHL),
                    Operand(id) => XorA(OpRegister(id)),
                },

                0xB0..=0xB7 => match operands[operand_idx] {
                    RegisterOperand::HL => OrA(OpHL),
                    Operand(id) => OrA(OpRegister(id)),
                },

                0xB8..=0xBF => match operands[operand_idx] {
                    RegisterOperand::HL => CpA(OpHL),
                    Operand(id) => CpA(OpRegister(id)),
                },

                0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                    match operands[(opcode as usize - 4) / 8] {
                        RegisterOperand::HL => InchHl,
                        Operand(id) => IncR8(id),
                    }
                }

                0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                    match operands[(opcode as usize - 5) / 8] {
                        RegisterOperand::HL => DechHl,
                        Operand(id) => DecR8(id),
                    }
                }

                0x36 => LdhHlU8(ram.read(pc[1])),

                0x0A => LdAR16(reg.bc()),
                0x1A => LdAR16(reg.de()),

                0xFA => LdhAU16(u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])])),

                0x3E => LdAU8(ram.read(pc[1])),

                0x02 => LdR16A(reg.bc()),
                0x12 => LdR16A(reg.de()),

                0xEA => LdhU16A(u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])])),

                0xF2 => LdhAC,
                0xE2 => LdhCA,

                0x3A => LdAHld,
                0x32 => LdHldA,
                0x2A => LdAHli,
                0x22 => LdHliA,

                0xE0 => LdhU8A(ram.read(pc[1])),
                0xF0 => LdhAU8(ram.read(pc[1])),

                0x01 => LdR16U16(
                    reg.bc(),
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),
                0x11 => LdR16U16(
                    reg.de(),
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),
                0x21 => LdR16U16(
                    reg.hl(),
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),
                0x31 => LdR16U16(
                    reg.sp,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),

                0xF9 => LdSpHl,
                0xF8 => LdHlSpI8(ram.read(pc[1]) as i8),

                0x08 => LdU16Sp(u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])])),

                0xF5 => PushAf,
                0xC5 => PushR16(reg.bc()),
                0xD5 => PushR16(reg.de()),
                0xE5 => PushR16(reg.hl()),

                0xC1 => PopR16(reg.bc()),
                0xD1 => PopR16(reg.de()),
                0xE1 => PopR16(reg.hl()),
                0xF1 => PopR16(reg.af()),

                0xC6 => AddA(OpByte(ram.read(pc[1]))),
                0xCE => AdcA(OpByte(ram.read(pc[1]))),
                0xD6 => SubA(OpByte(ram.read(pc[1]))),
                0xDE => SbcA(OpByte(ram.read(pc[1]))),
                0xE6 => AndA(OpByte(ram.read(pc[1]))),
                0xF6 => OrA(OpByte(ram.read(pc[1]))),
                0xEE => XorA(OpByte(ram.read(pc[1]))),
                0xFE => CpA(OpByte(ram.read(pc[1]))),

                0x09 => AddHlR16(reg.bc()),
                0x19 => AddHlR16(reg.de()),
                0x29 => AddHlR16(reg.hl()),
                0x39 => AddHlR16(reg.sp),

                0x03 => IncR16(reg.bc()),
                0x13 => IncR16(reg.de()),
                0x23 => IncR16(reg.hl()),
                0x33 => IncR16(reg.sp),

                0x0B => DecR16(reg.bc()),
                0x1B => DecR16(reg.de()),
                0x2B => DecR16(reg.hl()),
                0x3B => DecR16(reg.sp),

                0xE8 => AddSpI8(ram.read(pc[1]) as i8),

                0x27 => Daa,
                0x2F => Cpl,
                0x3F => Ccf,
                0x37 => Scf,
                0x00 => Nop,
                0x76 => Halt,
                0xF3 => DisableInterrupt,
                0xFB => EnableInterrupt,
                0x07 => Rlc(OpRegister(A), true),
                0x17 => Rl(OpRegister(A), true),
                0x0F => Rrc(OpRegister(A), true),
                0x1F => Rr(OpRegister(A), true),

                0x10 => {
                    let opcode = ram.internal_read(pc[1] as usize);
                    match opcode {
                        0x00 => Stop,
                        _ => panic!("Invalid opcode after STOP: {}", opcode),
                    }
                }

                0xC3 => JpU16(u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])])),
                0xC2 => JpCcU16(
                    ConditionCode::NZ,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),
                0xCA => JpCcU16(
                    ConditionCode::Z,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),
                0xD2 => JpCcU16(
                    ConditionCode::NC,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),

                0xDA => JpCcU16(
                    ConditionCode::C,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),
                0xE9 => JpHl,

                0x18 => JrI8(ram.read(pc[1]) as i8),
                0x20 => JrCcI8(ConditionCode::NZ, ram.read(pc[1]) as i8),
                0x28 => JrCcI8(ConditionCode::Z, ram.read(pc[1]) as i8),
                0x30 => JrCcI8(ConditionCode::NC, ram.read(pc[1]) as i8),
                0x38 => JrCcI8(ConditionCode::C, ram.read(pc[1]) as i8),
                0xCD => CallU16(u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])])),

                0xC4 => CallCcU16(
                    ConditionCode::NZ,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),

                0xCC => CallCcU16(
                    ConditionCode::Z,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),

                0xD4 => CallCcU16(
                    ConditionCode::NC,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),

                0xDC => CallCcU16(
                    ConditionCode::C,
                    u16::from_le_bytes([ram.read(pc[1]), ram.read(pc[2])]),
                ),

                0xC7 => Rst(RstVec::X00),

                0xCF => Rst(RstVec::X08),

                0xD7 => Rst(RstVec::X10),

                0xDF => Rst(RstVec::X18),

                0xE7 => Rst(RstVec::X20),

                0xEF => Rst(RstVec::X28),

                0xF7 => Rst(RstVec::X30),

                0xFF => Rst(RstVec::X38),

                0xC9 => Ret,

                0xC0 => RetCc(ConditionCode::NZ),

                0xC8 => RetCc(ConditionCode::Z),

                0xD0 => RetCc(ConditionCode::NC),

                0xD8 => RetCc(ConditionCode::C),

                0xD9 => Reti,

                0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {
                    panic!(
                        "P: {}, C: {}, N: {}",
                        ram.read(pc[0] - 1),
                        opcode,
                        ram.read(pc[1])
                    )
                }
            },
        )
    }
}
