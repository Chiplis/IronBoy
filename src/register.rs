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

impl Into<u16> for ByteRegister {
    fn into(self) -> u16 {
        self.0 as u16 + 0xFF00
    }
}

impl Into<u8> for ByteRegister {
    fn into(self) -> u8 { self.0 }
}

#[derive(Copy, Clone, Debug)]
pub struct FlagRegister {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl FlagRegister {
    pub fn value(&self) -> u8 {
        [self.c, self.h, self.n, self.z]
            .iter()
            .map(|f| if *f { 1 } else { 0 })
            .enumerate()
            .map(|(i, n)| (n << (i + 4)) as u8)
            .sum()
    }

    pub fn set(&mut self, v: u8) {
        self.z = 0x80 & v != 0;
        self.n = 0x40 & v != 0;
        self.h = 0x20 & v != 0;
        self.c = 0x10 & v != 0;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum WordRegister {
    Double(ByteRegister, ByteRegister),
    AccFlag(ByteRegister, FlagRegister),
    StackPointer(u16),
}


impl Into<u16> for WordRegister {
    fn into(self) -> u16 { self.to_address() }
}


impl WordRegister {
    pub(crate) fn to_address(self) -> u16 {
        match self {
            WordRegister::Double(h, l) => u16::from_le_bytes([l.0, h.0]),
            WordRegister::AccFlag(a, FlagRegister { z, n, h, c }) => {
                let bit_flag = |b: bool, v: u32| 2u8.pow(v) as u8 * if b { 1 } else { 0 };
                u16::from_le_bytes([bit_flag(z, 3) + bit_flag(n, 2) + bit_flag(h, 1) + bit_flag(c, 0), a.0])
            }
            WordRegister::StackPointer(n) => n
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
