use crate::Instruction;
use crate::register::{Bit, WordRegister, RegisterId, FlagRegister, ProgramCounter, ByteRegister, ConditionCode};
use crate::instruction::Instruction::*;
use crate::instruction::RstVec;
use crate::memory_map::MemoryMap;
use crate::register::WordRegister::StackPointer;
use std::iter::FromIterator;
use std::ops::Div;
use std::cmp::max;

enum RegisterOperand {
    HL,
    Byte(ByteRegister),
}

#[derive(Clone)]
pub struct Gameboy {
    pub i: u32,
    pub a: ByteRegister,
    pub b: ByteRegister,
    pub c: ByteRegister,
    pub d: ByteRegister,
    pub e: ByteRegister,
    pub h: ByteRegister,
    pub l: ByteRegister,
    pub f: FlagRegister,
    pub pc: ProgramCounter,
    pub sp: WordRegister,
    pub mem: MemoryMap,
    pub vram: [u8; 2 * 8 * 1024],
    pub ime_counter: i8,
    pub ime: bool,
}

trait Special {}

impl Special for (ByteRegister, ByteRegister) {}

impl Gameboy {
    pub fn af(self) -> WordRegister {
        WordRegister::AccFlag(self.a, self.f)
    }
    pub fn bc(&self) -> WordRegister {
        WordRegister::Double(self.b, self.c)
    }
    pub fn de(&self) -> WordRegister {
        WordRegister::Double(self.d, self.e)
    }

    pub fn hl(&self) -> WordRegister {
        WordRegister::Double(self.h, self.l)
    }

    pub fn set_word_register(&mut self, value: u16, reg: WordRegister) {
        let [lo, hi] = value.to_le_bytes();
        match reg {
            WordRegister::AccFlag(_, _) => {
                self.a.0 = hi;
                self.set_flag(lo);
            }
            WordRegister::Double(a, b) => {
                self.get_register(a.1).0 = hi;
                self.get_register(b.1).0 = lo;
            }
            WordRegister::StackPointer(_) => self.sp = StackPointer(value)
        };
    }

    pub fn cc_flag(&mut self, cc: ConditionCode) -> bool {
        match cc {
            ConditionCode::Z => self.f.z,
            ConditionCode::NZ => !self.f.z,
            ConditionCode::C => self.f.c,
            ConditionCode::NC => !self.f.c
        }
    }

    pub fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        self.f.z = z;
        self.f.n = n;
        self.f.c = c;
        self.f.h = h;
    }

    pub fn set_flag(&mut self, flag: u8) {
        let flags = flag & 0xF0;
        self.f.z = flags & 0x80 == 0x80;
        self.f.n = flags & 0x40 == 0x40;
        self.f.c = flags & 0x20 == 0x20;
        self.f.h = flags & 0x10 == 0x10;
    }

    pub fn get_register(&mut self, id: RegisterId) -> &mut ByteRegister {
        match id {
            RegisterId::A => &mut self.a,
            RegisterId::B => &mut self.b,
            RegisterId::C => &mut self.c,
            RegisterId::D => &mut self.d,
            RegisterId::E => &mut self.e,
            RegisterId::H => &mut self.h,
            RegisterId::L => &mut self.l,
        }
    }
}

#[deny(unreachable_patterns)]
pub fn fetch_instruction(gb: &Gameboy) -> (u8, Instruction) {
    let pc = gb.pc.0;
    let ram = &gb.mem;
    let opcode = ram[pc];
    let registers = [gb.b, gb.c, gb.d, gb.e, gb.h, gb.l, gb.a];

    let mut operands = Vec::from_iter(registers.iter().map(|r| RegisterOperand::Byte(*r)));
    operands.insert(operands.len() - 1, RegisterOperand::HL);
    let operand_idx = ((opcode & 0x0F) % 8) as usize;
    let mut register_idx = (max(0x40, opcode) as usize - 0x40) / 8;

    if (0x77_u8..0x80_u8).contains(&opcode) {
        operands.rotate_right(1);
        register_idx -= 1;
    }

    (opcode, match opcode {
        0xCB => {
            let cb_opcode = ram[pc + 1] as u8;

            let bit: usize = ((cb_opcode as usize % 0x40) >> 4) * 2 + if cb_opcode & 0x0F > 7 { 1 } else { 0 };
            if bit > 7 { panic!("Bit parsing is failing: {}.", bit) };

            let set = [128, 64, 32, 16, 8, 4, 2, 1];
            let res = [127, 191, 223, 239, 247, 251, 253, 254];
            let bit_idx = ((cb_opcode & 0x0F) % 8) as usize;

            match cb_opcode {
                0x00..=0x07 => match operands[bit_idx] {
                    RegisterOperand::HL => RLC_HL,
                    RegisterOperand::Byte(reg) => RLC_R8(reg),
                },

                0x08..=0x0F => match operands[bit_idx] {
                    RegisterOperand::HL => RRC_HL,
                    RegisterOperand::Byte(reg) => RRC_R8(reg),
                },

                0x10..=0x17 => match operands[bit_idx] {
                    RegisterOperand::HL => RL_HL,
                    RegisterOperand::Byte(reg) => RL_R8(reg),
                },

                0x18..=0x1F => match operands[bit_idx] {
                    RegisterOperand::HL => RR_HL,
                    RegisterOperand::Byte(reg) => RR_R8(reg),
                },

                0x20..=0x27 => match operands[bit_idx] {
                    RegisterOperand::HL => SLA_HL,
                    RegisterOperand::Byte(reg) => SLA_R8(reg),
                },

                0x28..=0x2F => match operands[bit_idx] {
                    RegisterOperand::HL => SRA_HL,
                    RegisterOperand::Byte(reg) => SRA_R8(reg),
                },

                0x30..=0x37 => match operands[bit_idx] {
                    RegisterOperand::HL => SWAP_HL,
                    RegisterOperand::Byte(reg) => SWAP_R8(reg),
                },

                0x38..=0x3F => match operands[bit_idx] {
                    RegisterOperand::HL => SRL_HL,
                    RegisterOperand::Byte(reg) => SRL_R8(reg),
                },
                0x40..=0x7F => match operands[bit_idx] {
                    RegisterOperand::HL => BIT_U3_HL(Bit(set[bit])),
                    RegisterOperand::Byte(reg) => BIT_U3_R8(Bit(set[bit]), reg)
                },

                0x80..=0xBF => match operands[bit_idx] {
                    RegisterOperand::HL => RES_U3_HL(Bit(res[bit])),
                    RegisterOperand::Byte(reg) => RES_U3_R8(Bit(res[bit]), reg),
                },

                0xC0..=0xFF => match operands[bit_idx] {
                    RegisterOperand::HL => SET_U3_HL(Bit(set[bit])),
                    RegisterOperand::Byte(reg) => SET_U3_R8(Bit(set[bit]), reg),
                },
            }
        }

        0x06 => LD_R8_N8(gb.b, ram[pc + 1]),
        0x0E => LD_R8_N8(gb.c, ram[pc + 1]),
        0x16 => LD_R8_N8(gb.d, ram[pc + 1]),
        0x1E => LD_R8_N8(gb.e, ram[pc + 1]),
        0x26 => LD_R8_N8(gb.h, ram[pc + 1]),
        0x2E => LD_R8_N8(gb.l, ram[pc + 1]),

        0x40..=0x75 | 0x77..=0x7F => match operands[operand_idx] {
            RegisterOperand::HL => LD_R8_HL(registers[register_idx]),
            RegisterOperand::Byte(reg) => LD_R8_R8(registers[register_idx], reg)
        },

        0x80..=0x87 => match operands[operand_idx] {
            RegisterOperand::HL => ADD_A_HL,
            RegisterOperand::Byte(reg) => ADD_A_R8(reg),
        },

        0x88..=0x8F => match operands[operand_idx] {
            RegisterOperand::HL => ADC_A_HL,
            RegisterOperand::Byte(reg) => ADC_A_R8(reg),
        },

        0x90..=0x97 => match operands[operand_idx] {
            RegisterOperand::HL => SUB_A_HL,
            RegisterOperand::Byte(reg) => SUB_A_R8(reg),
        },

        0x98..=0x9F => match operands[operand_idx] {
            RegisterOperand::HL => SBC_A_HL,
            RegisterOperand::Byte(reg) => SBC_A_R8(reg),
        },

        0xA0..=0xA7 => match operands[operand_idx] {
            RegisterOperand::HL => AND_A_HL,
            RegisterOperand::Byte(reg) => AND_A_R8(reg),
        },

        0xA8..=0xAF => match operands[operand_idx] {
            RegisterOperand::HL => XOR_A_HL,
            RegisterOperand::Byte(reg) => XOR_A_R8(reg),
        },

        0xB0..=0xB7 => match operands[operand_idx] {
            RegisterOperand::HL => OR_A_HL,
            RegisterOperand::Byte(reg) => OR_A_R8(reg),
        },

        0xB8..=0xBF => match operands[operand_idx] {
            RegisterOperand::HL => CP_A_HL,
            RegisterOperand::Byte(reg) => CP_A_R8(reg),
        },

        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
            match operands[(opcode as usize - 4) / 8] {
                RegisterOperand::HL => INC_R16(gb.hl()),
                RegisterOperand::Byte(reg) => INC_R8(reg),
            }
        }

        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
            match operands[(opcode as usize - 5) / 8] {
                RegisterOperand::HL => DEC_R16(gb.hl()),
                RegisterOperand::Byte(reg) => DEC_R8(reg),
            }
        }

        0x36 => LDH_HL_N8(ram[pc + 1]),

        0x0A => LD_A_R16(gb.bc()),
        0x1A => LD_A_R16(gb.de()),

        0xFA => LD_A_N16(u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0x3E => LD_A_N8(ram[pc + 1]),

        0x02 => LD_R16_A(gb.bc()),
        0x12 => LD_R16_A(gb.de()),

        0xEA => LDH_N16_A(u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xF2 => LDH_A_C,
        0xE2 => LDH_C_A,

        0x3A => LD_A_HLD,
        0x32 => LD_HLD_A,
        0x2A => LD_A_HLI,
        0x22 => LD_HLI_A,

        0xE0 => LDH_N8_A(ram[pc + 1]),
        0xF0 => LDH_A_N8(ram[pc + 1]),

        0x01 => LD_R16_N16(gb.bc(), u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0x11 => LD_R16_N16(gb.de(), u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0x21 => LD_R16_N16(gb.hl(), u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0x31 => LD_R16_N16(gb.hl(), u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xF9 => LD_SP_HL,
        0xF8 => LD_HL_SP_E8(ram[pc + 1] as i8),

        0x08 => LD_N16_SP(u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xF5 => PUSH_AF,
        0xC5 => PUSH_R16(gb.bc()),
        0xD5 => PUSH_R16(gb.de()),
        0xE5 => PUSH_R16(gb.hl()),

        0xF1 => POP_AF,
        0xC1 => POP_R16(gb.bc()),
        0xD1 => POP_R16(gb.de()),
        0xE1 => POP_R16(gb.hl()),

        0xC6 => ADD_A_N8(ram[pc + 1]),
        0xCE => ADC_A_N8(ram[pc + 1]),
        0xD6 => SUB_A_N8(ram[pc + 1]),
        0xDE => SBC_A_N8(ram[pc + 1]),
        0xE6 => AND_A_N8(ram[pc + 1]),
        0xF6 => OR_A_N8(ram[pc + 1]),
        0xEE => XOR_A_N8(ram[pc + 1]),
        0xFE => CP_A_N8(ram[pc + 1]),

        0x09 => ADD_HL_R16(gb.bc()),
        0x19 => ADD_HL_R16(gb.de()),
        0x29 => ADD_HL_R16(gb.hl()),
        0x39 => ADD_HL_R16(gb.sp),

        0x03 => INC_R16(gb.bc()),
        0x13 => INC_R16(gb.de()),
        0x23 => INC_R16(gb.hl()),
        0x33 => INC_R16(gb.sp),

        0x0B => DEC_R16(gb.bc()),
        0x1B => DEC_R16(gb.de()),
        0x2B => DEC_R16(gb.hl()),
        0x3B => DEC_R16(gb.sp),

        0xE8 => ADD_SP_E8(ram[pc + 1] as i8),

        0x27 => DAA,
        0x2F => CPL,
        0x3F => CCF,
        0x37 => SCF,
        0x00 => NOP,
        0x76 => HALT,
        0xF3 => DI,
        0xFB => EI,
        0x07 => RLCA,
        0x17 => RLA,
        0x0F => RRCA,
        0x1F => RRA,

        0x10 => {
            let opcode = ram[pc + 1];
            match opcode {
                0x00 => STOP,
                _ => panic!("Invalid opcode after STOP: {}", opcode),
            }
        }

        0xC3 => JP_N16(u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0xC2 => JP_CC_N16(ConditionCode::NZ, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0xCA => JP_CC_N16(ConditionCode::Z, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0xD2 => JP_CC_N16(ConditionCode::NC, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xDA => JP_CC_N16(ConditionCode::C, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),
        0xE9 => JP_HL,

        0x18 => JR_E8(ram[pc + 1] as i8),
        0x20 => JR_CC_E8(ConditionCode::NZ, ram[pc + 1] as i8),
        0x28 => JR_CC_E8(ConditionCode::Z, ram[pc + 1] as i8),
        0x30 => JR_CC_E8(ConditionCode::NC, ram[pc + 1] as i8),
        0x38 => JR_CC_E8(ConditionCode::C, ram[pc + 1] as i8),
        0xCD => CALL_N16(u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xC4 => CALL_CC_N16(ConditionCode::NZ, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xCC => CALL_CC_N16(ConditionCode::Z, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xD4 => CALL_CC_N16(ConditionCode::NC, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xDC => CALL_CC_N16(ConditionCode::C, u16::from_le_bytes([ram[pc + 1], ram[pc + 2]])),

        0xC7 => RST(RstVec::X00),

        0xCF => RST(RstVec::X08),

        0xD7 => RST(RstVec::X10),

        0xDF => RST(RstVec::X18),

        0xE7 => RST(RstVec::X20),

        0xEF => RST(RstVec::X28),

        0xF7 => RST(RstVec::X30),

        0xFF => RST(RstVec::X38),

        0xC9 => RET,

        0xC0 => RET_CC(ConditionCode::NZ),

        0xC8 => RET_CC(ConditionCode::Z),

        0xD0 => RET_CC(ConditionCode::NC),

        0xD8 => RET_CC(ConditionCode::C),

        0xD9 => RETI,

        0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {
            panic!("P: {}, C: {}, N: {}", ram[pc - 1], opcode, ram[pc + 1])
        }
    })
}