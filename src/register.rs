#[derive(Copy, Clone, Debug)]
pub enum RegisterId {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Copy, Clone, Debug)]
pub struct ByteRegister(pub u8, pub RegisterId);

#[derive(Copy, Clone, Debug)]
pub struct FlagRegister {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum SpecialRegister {
    WordRegister(ByteRegister, ByteRegister),
    DoubleFlagRegister(ByteRegister, FlagRegister),
    StackPointer(u16),
}

impl SpecialRegister {
    pub fn value(self) -> u16 {
        match self {
            SpecialRegister::WordRegister(h, l) => u16::from_le_bytes([l.0, h.0]),
            SpecialRegister::DoubleFlagRegister(a, FlagRegister { z, n, h, c }) => {
                let bit_flag = |b: bool, v: u32| 2u8.pow(v) as u8 * if b { 1 } else { 0 };
                u16::from_le_bytes([bit_flag(z, 3) + bit_flag(n, 2) + bit_flag(h, 1) + bit_flag(c, 0), a.0])
            }
            SpecialRegister::StackPointer(n) => n
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Bit(pub u8);

#[derive(Copy, Clone, Debug)]
pub struct ProgramCounter(pub u16);

#[derive(Copy, Clone, Debug)]
pub enum ConditionCode {
    Z,
    NZ,
    C,
    NC,
}
