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
pub struct FlagRegister{pub z: bool, pub n: bool, pub h: bool, pub c: bool}

#[derive(Copy, Clone, Debug)]
pub enum SpecialRegister {
    WordRegister(ByteRegister, ByteRegister),
    DoubleFlagRegister(ByteRegister, FlagRegister),
}

impl SpecialRegister {
    pub fn value(self) -> u16 {
        match self  {
            SpecialRegister::WordRegister(h, l) => SpecialRegister::merge(h.0, l.0),
            SpecialRegister::DoubleFlagRegister(a, FlagRegister{z, n, h, c}) => {
                let bit_flag = |b: bool, v: u8 | if b { 2u8.pow(v as u32) as u8 } else { 0 };
                SpecialRegister::merge(a.0, bit_flag(z, 3) + bit_flag(n, 2) + bit_flag(h, 1) + bit_flag(c, 0))
            },
        }
    }

    fn merge(a: u8, b: u8) -> u16 {
        ((b as u16) << 8) | a as u16
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Bit(pub u8);

#[derive(Copy, Clone, Debug)]
pub struct ProgramCounter(pub u16);

#[derive(Copy, Clone, Debug)]
pub struct StackPointer(pub u16);

#[derive(Copy, Clone, Debug)]
pub enum ConditionCode {
    Z,
    NZ,
    C,
    NC,
}
