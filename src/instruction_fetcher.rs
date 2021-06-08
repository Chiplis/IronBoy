use crate::Instruction;
use crate::register::{Bit, SpecialRegister, RegisterId, FlagRegister, ProgramCounter, SimpleRegister, StackPointer, ConditionCode};
use crate::instruction::Instruction::*;
use crate::instruction::RstVec;

enum RegisterOperand {
    Special(SpecialRegister),
    Simple(SimpleRegister),
}

#[derive(Clone)]
pub struct Gameboy {
    pub a: SimpleRegister,
    pub b: SimpleRegister,
    pub c: SimpleRegister,
    pub d: SimpleRegister,
    pub e: SimpleRegister,
    pub h: SimpleRegister,
    pub l: SimpleRegister,
    pub f: FlagRegister,
    pub pc: ProgramCounter,
    pub sp: StackPointer,
    pub ram: [u8; 0x10000],
    pub vram: [u8; 2 * 8 * 1024],
    pub rom: Vec<u8>,
}

impl Gameboy {
    pub fn af(&self) -> SpecialRegister {
        SpecialRegister::DoubleFlagRegister(self.a, self.f)
    }
    pub fn bc(&self) -> SpecialRegister {
        SpecialRegister::DoubleRegister(self.b, self.c)
    }
    pub fn de(&self) -> SpecialRegister {
        SpecialRegister::DoubleRegister(self.d, self.e)
    }

    pub fn hl(&self) -> SpecialRegister {
        SpecialRegister::DoubleRegister(self.h, self.l)
    }

    pub fn set_double_register(&mut self, value: u16, reg: SpecialRegister) {
        let [hi, lo] = value.to_le_bytes();
        match reg {
            SpecialRegister::DoubleFlagRegister(_, _) => {
                self.a.0 = hi;
                self.set_flag(lo);
            }
            SpecialRegister::DoubleRegister(a, b) => {
                self.get_register(a.1).0 = hi;
                self.get_register(b.1).0 = lo;
            }
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

    pub fn get_register(&mut self, id: RegisterId) -> &mut SimpleRegister {
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

fn merge(a: u8, b: u8) -> u16 {
    ((b as u16) << 8) | a as u16
}

#[deny(unreachable_patterns)]
pub fn fetch_instruction(gameboy: &Gameboy) -> Instruction {
    let pc = gameboy.pc.0 as usize;
    let rom = &gameboy.rom;
    let value = rom[pc];
    print!("{:#04x}", value);
    let r = [
        RegisterOperand::Simple(gameboy.b),
        RegisterOperand::Simple(gameboy.c),
        RegisterOperand::Simple(gameboy.d),
        RegisterOperand::Simple(gameboy.e),
        RegisterOperand::Simple(gameboy.h),
        RegisterOperand::Simple(gameboy.l),
        RegisterOperand::Special(gameboy.hl()),
        RegisterOperand::Simple(gameboy.a),
    ];
    let r_idx = ((value & 0x0F) % 8) as usize;
    match value {
        // Redundant self assignments
        0xCB => {
            let cb_opcode = rom[pc+1];

            let bit: usize = (((cb_opcode % 0x40) >> 4) * 2 + if cb_opcode & 0x0F > 7 { 1 } else { 0 }) as usize;
            if bit > 7 {
                panic!("Bit parsing is failing: {}.", bit)
            };
            let set = [128, 64, 32, 16, 8, 4, 2, 1];
            let res = [127, 191, 223, 239, 247, 251, 253, 254];
            let bit_idx = ((cb_opcode & 0x0F) % 8) as usize;

            match cb_opcode {
                0x00..=0x07 => match r[bit_idx] {
                    RegisterOperand::Special(_) => RLC_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => RLC_R8(reg),
                },

                0x08..=0x0F => match r[bit_idx] {
                    RegisterOperand::Special(_) => RRC_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => RRC_R8(reg),
                },

                0x10..=0x17 => match r[bit_idx] {
                    RegisterOperand::Special(_) => RL_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => RL_R8(reg),
                },

                0x18..=0x1F => match r[bit_idx] {
                    RegisterOperand::Special(_) => RR_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => RR_R8(reg),
                },

                0x20..=0x27 => match r[bit_idx] {
                    RegisterOperand::Special(_) => SLA_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => SLA_R8(reg),
                },

                0x28..=0x2F => match r[bit_idx] {
                    RegisterOperand::Special(_) => SRA_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => SRA_R8(reg),
                },

                0x30..=0x37 => match r[bit_idx] {
                    RegisterOperand::Special(_) => SWAP_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => SWAP_R8(reg),
                },

                0x38..=0x3F => match r[bit_idx] {
                    RegisterOperand::Special(_) => SRL_HL(gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => SRL_R8(reg),
                },
                0x40..=0x7F => match r[bit_idx] {
                    RegisterOperand::Special(_) => BIT_U3_HL(Bit(set[bit]), gameboy.ram[gameboy.hl().value() as usize]),
                    RegisterOperand::Simple(reg) => BIT_U3_R8(Bit(set[bit]), reg),
                },

                0x80..=0xBF => match r[bit_idx] {
                    RegisterOperand::Special(_) => RES_U3_HL(Bit(res[bit]), gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => RES_U3_R8(Bit(res[bit]), reg),
                },

                0xC0..=0xFF => match r[bit_idx] {
                    RegisterOperand::Special(_) => SET_U3_HL(Bit(set[bit]), gameboy.hl().value()),
                    RegisterOperand::Simple(reg) => SET_U3_R8(Bit(set[bit]), reg),
                },
            }
        }

        0x06 => LD_R8_N8(gameboy.b, rom[pc+1]),
        0x0E => LD_R8_N8(gameboy.c, rom[pc+1]),
        0x16 => LD_R8_N8(gameboy.d, rom[pc+1]),
        0x1E => LD_R8_N8(gameboy.e, rom[pc+1]),
        0x26 => LD_R8_N8(gameboy.h, rom[pc+1]),
        0x2E => LD_R8_N8(gameboy.l, rom[pc+1]),

        0x78..=0x7F => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.a, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.a, reg),
        },

        0x40..=0x47 => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.b, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.b, reg),
        },

        0x48..=0x4F => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.c, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.c, reg),
        },

        0x50..=0x57 => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.d, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.d, reg),
        },

        0x58..=0x5F => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.e, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.e, reg),
        },

        0x60..=0x67 => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.h, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.h, reg),
        },

        0x68..=0x6F => match r[r_idx] {
            RegisterOperand::Special(_) => LD_R8_HL(gameboy.l, gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => LD_R8_R8(gameboy.l, reg),
        },

        0x70..=0x75 => match r[r_idx] {
            RegisterOperand::Simple(reg) => LD_HL_R8(gameboy.hl().value(), reg),
            _ => panic!("Should never use HL register for these opcodes."),
        },

        0x80..=0x87 => match r[r_idx] {
            RegisterOperand::Special(_) => ADD_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => ADD_A_R8(reg),
        },

        0x88..=0x8F => match r[r_idx] {
            RegisterOperand::Special(_) => ADC_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => ADC_A_R8(reg),
        },

        0x90..=0x97 => match r[r_idx] {
            RegisterOperand::Special(_) => SUB_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => SUB_A_R8(reg),
        },

        0x98..=0x9F => match r[r_idx] {
            RegisterOperand::Special(_) => SBC_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => SBC_A_R8(reg),
        },

        0xA0..=0xA7 => match r[r_idx] {
            RegisterOperand::Special(_) => AND_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => AND_A_R8(reg),
        },

        0xA8..=0xAF => match r[r_idx] {
            RegisterOperand::Special(_) => XOR_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => XOR_A_R8(reg),
        },

        0xB0..=0xB7 => match r[r_idx] {
            RegisterOperand::Special(_) => OR_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => OR_A_R8(reg),
        },

        0xB8..=0xBF => match r[r_idx] {
            RegisterOperand::Special(_) => CP_A_HL(gameboy.ram[gameboy.hl().value() as usize]),
            RegisterOperand::Simple(reg) => CP_A_R8(reg),
        },

        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
            match r[(value as usize - 4) / 8] {
                RegisterOperand::Special(_) => INC_HL(gameboy.hl().value()),
                RegisterOperand::Simple(reg) => INC_R8(reg),
            }
        }

        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
            match r[(value as usize - 5) / 8] {
                RegisterOperand::Special(_) => DEC_HL(gameboy.hl().value()),
                RegisterOperand::Simple(reg) => DEC_R8(reg),
            }
        }

        0x36 => LD_HL_N8(gameboy.hl().value(), rom[pc+1]),

        0x0A => LD_A_R16(gameboy.bc()),
        0x1A => LD_A_R16(gameboy.de()),

        0xFA => LD_A_N16(merge(rom[pc + 1], rom[pc + 2])),

        0x3E => LD_N16_A(merge(rom[pc + 1], rom[pc + 2])),

        0x02 => LD_R16_A(gameboy.bc()),
        0x12 => LD_R16_A(gameboy.de()),
        0x77 => LD_R16_A(gameboy.hl()),

        0xEA => LD_N16_A(merge(rom[pc + 1], rom[pc + 2])),

        0xF2 => LDH_A_C,
        0xE2 => LDH_C_A,

        0x3A => LD_A_HLD(gameboy.hl().value()),
        0x32 => LD_HLD_A(gameboy.hl().value()),
        0x2A => LD_A_HLI(gameboy.hl().value()),
        0x22 => LD_HLI_A(gameboy.hl().value()),

        0xE0 => LDH_N8_A(rom[pc+1]),
        0xF0 => LDH_A_N8(rom[pc+1]),

        0x01 => LD_R16_N16(gameboy.bc(), merge(rom[pc + 1], rom[pc + 2])),
        0x11 => LD_R16_N16(gameboy.de(), merge(rom[pc + 1], rom[pc + 2])),
        0x21 => LD_R16_N16(gameboy.hl(), merge(rom[pc + 1], rom[pc + 2])),
        0x31 => LD_SP_N16(merge(rom[pc + 1], rom[pc + 2])),

        0xF9 => LD_SP_HL(gameboy.hl().value()),
        0xF8 => LD_HL_SP_E8(rom[pc+1] as i8),

        0x08 => LD_N16_SP(merge(rom[pc + 1], rom[pc + 2])),

        0xF5 => PUSH_AF,
        0xC5 => PUSH_R16(gameboy.bc()),
        0xD5 => PUSH_R16(gameboy.de()),
        0xE5 => PUSH_R16(gameboy.hl()),

        0xF1 => POP_AF,
        0xC1 => POP_R16(gameboy.bc()),
        0xD1 => POP_R16(gameboy.de()),
        0xE1 => POP_R16(gameboy.hl()),

        0xC6 => ADD_A_N8(rom[pc+1]),
        0xCE => ADC_A_N8(rom[pc+1]),
        0xD6 => SUB_A_N8(rom[pc+1]),
        0xDE => SBC_A_N8(rom[pc+1]),
        0xE6 => AND_A_N8(rom[pc+1]),
        0xF6 => OR_A_N8(rom[pc+1]),
        0xEE => XOR_A_N8(rom[pc+1]),
        0xFE => CP_A_N8(rom[pc+1]),

        0x09 => ADD_HL_R16(gameboy.bc()),
        0x19 => ADD_HL_R16(gameboy.de()),
        0x29 => ADD_HL_R16(gameboy.hl()),
        0x39 => ADD_HL_SP(gameboy.hl().value(), gameboy.sp.0),

        0x03 => INC_R16(gameboy.bc()),
        0x13 => INC_R16(gameboy.de()),
        0x23 => INC_R16(gameboy.hl()),
        0x33 => INC_SP(gameboy.sp.0),

        0x0B => DEC_R16(gameboy.bc()),
        0x1B => DEC_R16(gameboy.de()),
        0x2B => DEC_R16(gameboy.hl()),
        0x3B => DEC_SP(gameboy.sp.0),

        0xE8 => ADD_SP_E8(rom[pc+1] as i8),

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
            let opcode = rom[pc+1];
            match opcode {
                0x00 => STOP,
                _ => panic!("Invalid opcode after STOP: {}", value),
            }
        }

        0xC3 => JP_N16(merge(rom[pc + 1], rom[pc + 2])),
        0xC2 => JP_CC_N16(ConditionCode::NZ, merge(rom[pc + 1], rom[pc + 2])),
        0xCA => JP_CC_N16(ConditionCode::Z, merge(rom[pc + 1], rom[pc + 2])),
        0xD2 => JP_CC_N16(ConditionCode::NC, merge(rom[pc + 1], rom[pc + 2])),

        0xDA => JP_CC_N16(ConditionCode::C, merge(rom[pc + 1], rom[pc + 2])),
        0xE9 => JP_HL(gameboy.hl().value()),

        0x18 => JR_E8(rom[pc+1] as i8),
        0x20 => JR_CC_E8(ConditionCode::NZ, rom[pc+1] as i8),
        0x28 => JR_CC_E8(ConditionCode::Z, rom[pc+1] as i8),
        0x30 => JR_CC_E8(ConditionCode::NC, rom[pc+1] as i8),
        0x38 => JR_CC_E8(ConditionCode::C, rom[pc+1] as i8),
        0xCD => CALL_N16(merge(rom[pc + 1], rom[pc + 2])),

        0xC4 => CALL_CC_N16(ConditionCode::NZ, merge(rom[pc + 1], rom[pc + 2])),

        0xCC => CALL_CC_N16(ConditionCode::Z, merge(rom[pc + 1], rom[pc + 2])),

        0xD4 => CALL_CC_N16(ConditionCode::NC, merge(rom[pc + 1], rom[pc + 2])),

        0xDC => CALL_CC_N16(ConditionCode::C, merge(rom[pc + 1], rom[pc + 2])),

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
            panic!("P: {}, C: {}, N: {}", rom[pc - 1], value, rom[pc + 1])
        }
    }
}