#[derive(Debug)]
pub struct RType(u32);
impl RType {
    pub fn rs1(&self) -> u32 {
        (self.0 >> 20) & 0x1f
    }
    pub fn rs2(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x3
    }
    pub fn func7(&self) -> u32 {
        (self.0 >> 25) & 0x3f
    }
}

#[derive(Debug)]
pub struct IType(u32);
impl IType {
    pub fn rs1(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    pub fn imm11_0(&self) -> u32 {
        self.0 >> 20
    }
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x3
    }
}

#[derive(Debug)]
pub struct SType(u32);
impl SType {
    pub fn rs1(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    pub fn rs2(&self) -> u32 {
        (self.0 >> 20) & 0x1f
    }
    pub fn imm4_0(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    pub fn imm11_5(&self) -> u32 {
        self.0 >> 25
    }
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x3
    }
}

#[derive(Debug)]
pub struct BType(u32);
impl BType {
    pub fn rs1(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    pub fn rs2(&self) -> u32 {
        (self.0 >> 20) & 0x1f
    }
    pub fn imm11(&self) -> u32 {
        (self.0 >> 7) & 0x1
    }
    pub fn imm4_1(&self) -> u32 {
        (self.0 >> 8) & 0xf
    }
    pub fn imm10_5(&self) -> u32 {
        (self.0 >> 25) & 0x3f
    }
    pub fn imm12(&self) -> u32 {
        self.0 >> 31
    }
}

#[derive(Debug)]
pub struct UType(u32);
impl UType {
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    pub fn imm31_12(&self) -> u32 {
        self.0 >> 12
    }
}

#[derive(Debug)]
pub struct JType(u32);
impl JType {
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    pub fn imm19_12(&self) -> u32 {
        (self.0 >> 12) & 0xff
    }
    pub fn imm11(&self) -> u32 {
        (self.0 >> 20) & 0x1
    }
    pub fn imm10_1(&self) -> u32 {
        (self.0 >> 21) & 0x3ff
    }
    pub fn imm20(&self) -> u32 {
        self.0 >> 31
    }
}
