pub struct RType(pub u32);
impl RType {
    #[inline(always)]
    pub fn rs1(&self) -> u32 {
        (self.0 >> 20) & 0x1f
    }
    #[inline(always)]
    pub fn rs2(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    #[inline(always)]
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    #[inline(always)]
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x7
    }
    #[inline(always)]
    pub fn func7(&self) -> u32 {
        (self.0 >> 25) & 0x7f
    }
}

pub struct IType(pub u32);
impl IType {
    #[inline(always)]
    pub fn rs1(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    #[inline(always)]
    pub fn imm11_0(&self) -> u32 {
        self.0 >> 20
    }
    #[inline(always)]
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    #[inline(always)]
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x7
    }
}

pub struct SType(pub u32);
impl SType {
    #[inline(always)]
    pub fn rs1(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    #[inline(always)]
    pub fn rs2(&self) -> u32 {
        (self.0 >> 20) & 0x1f
    }
    #[inline(always)]
    pub fn imm4_0(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    #[inline(always)]
    pub fn imm11_5(&self) -> u32 {
        self.0 >> 25
    }
    #[inline(always)]
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x7
    }
}

pub struct BType(pub u32);
impl BType {
    #[inline(always)]
    pub fn func3(&self) -> u32 {
        (self.0 >> 12) & 0x7
    }
    #[inline(always)]
    pub fn rs1(&self) -> u32 {
        (self.0 >> 15) & 0x1f
    }
    #[inline(always)]
    pub fn rs2(&self) -> u32 {
        (self.0 >> 20) & 0x1f
    }
    #[inline(always)]
    pub fn imm11(&self) -> u32 {
        (self.0 >> 7) & 0x1
    }
    #[inline(always)]
    pub fn imm4_1(&self) -> u32 {
        (self.0 >> 8) & 0xf
    }
    #[inline(always)]
    pub fn imm10_5(&self) -> u32 {
        (self.0 >> 25) & 0x3f
    }
    #[inline(always)]
    pub fn imm12(&self) -> u32 {
        self.0 >> 31
    }
}

pub struct UType(pub u32);
impl UType {
    #[inline(always)]
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    #[inline(always)]
    pub fn imm31_12(&self) -> u32 {
        self.0 >> 12
    }
}

pub struct JType(pub u32);
impl JType {
    #[inline(always)]
    pub fn rd(&self) -> u32 {
        (self.0 >> 7) & 0x1f
    }
    #[inline(always)]
    pub fn imm19_12(&self) -> u32 {
        (self.0 >> 12) & 0xff
    }
    #[inline(always)]
    pub fn imm11(&self) -> u32 {
        (self.0 >> 20) & 0x1
    }
    #[inline(always)]
    pub fn imm10_1(&self) -> u32 {
        (self.0 >> 21) & 0x3ff
    }
    #[inline(always)]
    pub fn imm20(&self) -> u32 {
        self.0 >> 31
    }
}
