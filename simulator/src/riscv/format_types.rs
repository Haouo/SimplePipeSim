#[inline(always)]
fn sign_ext(data: u32, bit_widtgh: usize) -> u32 {
    assert!(bit_widtgh <= 32);
    let shamt = 32 - bit_widtgh;
    let sign_ext_data: i32 = ((data << shamt) as i32) >> shamt;
    sign_ext_data as u32
}

#[derive(Clone, Copy)]
pub struct RType(pub u32);
impl RType {
    #[inline(always)]
    pub fn rs1(&self) -> u8 {
        ((self.0 >> 20) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn rs2(&self) -> u8 {
        ((self.0 >> 15) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn rd(&self) -> u8 {
        ((self.0 >> 7) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn func3(&self) -> u8 {
        ((self.0 >> 12) & 0x7) as u8
    }
    #[inline(always)]
    pub fn func7(&self) -> u8 {
        ((self.0 >> 25) & 0x7f) as u8
    }
}

#[derive(Clone, Copy)]
pub struct IType(pub u32);
impl IType {
    #[inline(always)]
    pub fn rs1(&self) -> u8 {
        ((self.0 >> 15) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn imm11_0(&self) -> u32 {
        self.0 >> 20
    }
    #[inline(always)]
    pub fn rd(&self) -> u8 {
        ((self.0 >> 7) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn func3(&self) -> u8 {
        ((self.0 >> 12) & 0x7) as u8
    }
    #[inline(always)]
    pub fn imm_sign_ext(&self) -> u32 {
        sign_ext(self.imm11_0(), 12)
    }
}

#[derive(Clone, Copy)]
pub struct SType(pub u32);
impl SType {
    #[inline(always)]
    pub fn rs1(&self) -> u8 {
        ((self.0 >> 15) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn rs2(&self) -> u8 {
        ((self.0 >> 20) & 0x1f) as u8
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
    pub fn func3(&self) -> u8 {
        ((self.0 >> 12) & 0x7) as u8
    }
    #[inline(always)]
    pub fn sign_ext(&self) -> u32 {
        let origin_imm = (self.imm11_5() << 5) + self.imm4_0();
        sign_ext(origin_imm, 12)
    }
}

#[derive(Clone, Copy)]
pub struct BType(pub u32);
impl BType {
    #[inline(always)]
    pub fn func3(&self) -> u8 {
        ((self.0 >> 12) & 0x7) as u8
    }
    #[inline(always)]
    pub fn rs1(&self) -> u8 {
        ((self.0 >> 15) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn rs2(&self) -> u8 {
        ((self.0 >> 20) & 0x1f) as u8
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
    #[inline(always)]
    pub fn sign_ext(&self) -> u32 {
        let origin_imm = (self.imm12() << 12)
            + (self.imm11() << 11)
            + (self.imm10_5() << 5)
            + (self.imm4_1() << 1);
        sign_ext(origin_imm, 13)
    }
}

#[derive(Clone, Copy)]
pub struct UType(pub u32);
impl UType {
    #[inline(always)]
    pub fn rd(&self) -> u8 {
        ((self.0 >> 7) & 0x1f) as u8
    }
    #[inline(always)]
    pub fn imm31_12(&self) -> u32 {
        self.0 >> 12
    }
    #[inline(always)]
    pub fn sign_ext(&self) -> u32 {
        self.imm31_12() << 12
    }
}

#[derive(Clone, Copy)]
pub struct JType(pub u32);
impl JType {
    #[inline(always)]
    pub fn rd(&self) -> u8 {
        ((self.0 >> 7) & 0x1f) as u8
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
    #[inline(always)]
    pub fn sign_ext(&self) -> u32 {
        let origin_imm = (self.imm20() << 12)
            + (self.imm19_12() << 12)
            + (self.imm11() << 11)
            + (self.imm10_1() << 1);
        sign_ext(origin_imm, 21)
    }
}
