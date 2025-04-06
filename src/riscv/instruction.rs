use crate::pipeline::uop::{IndexValuePair, UOp};
use crate::riscv::types::*;

pub struct InstIdentifier {
    pub name: &'static str,
    pub mask: u32,
    pub data: u32
}

// @TODO: build a pre-defined inst. table for all implemented inst.
// const ALL_INSTRUCTION_DEF: [InstIdentifier: 46] =

// #[repr(u32)]
pub enum Instruction {
    /// ### LUI
    Lui(UType),
    /// ### AUIPC
    Auipc(UType),
    /// ### JAL
    Jal(JType),
    /// ### JALR
    Jalr(IType),
    /// ### BRANCH
    Beq(BType),
    Bne(BType),
    Blt(BType),
    Bge(BType),
    Bltu(BType),
    Bgeu(BType),
    /// ### LOAD
    Lb(IType),
    Lh(IType),
    Lw(IType),
    Lbu(IType),
    Lhu(IType),
    /// ### STORE
    Sb(SType),
    Sh(SType),
    Sw(SType),
    /// ### OP-IMM
    Addi(IType),
    Slti(IType),
    Sltiu(IType),
    Xori(IType),
    Ori(IType),
    Andi(IType),
    Slli(IType),
    Srli(IType),
    Srai(IType),
    /// ### OP
    ///
    /// It includes both base integer instructions and M-extension instructions
    Add(RType),
    Sub(RType),
    Sll(RType),
    Slt(RType),
    Sltu(RType),
    Xor(RType),
    Srl(RType),
    Sra(RType),
    Or(RType),
    And(RType),
    Mul(RType),
    Mulh(RType),
    Mulhsu(RType),
    Mulhu(RType),
    Div(RType),
    Divu(RType),
    Rem(RType),
    Remu(RType),
    /// SYSTEM
    Fence(IType),
    Ecall(IType),
    Ebreak(IType), // EBREAK is used to support smeihosting
}

impl Instruction {
    pub fn gen_uop(&self) -> UOp {
        let mut uop = UOp::default();
        match self {
            // LUI
            Self::Lui(utype) => {
                // TODO
            }
            // AUIPC
            Self::Auipc(u_type) => {}
            // JAL
            Self::Jal(j_type) => {}
            // JALR
            Self::Jalr(i_type) => {}
            // BRANCH
            Self::Beq(b_type) => {}
            Self::Bne(b_type) => {}
            Self::Blt(b_type) => {}
            Self::Bge(b_type) => {}
            Self::Bltu(b_type) => {}
            Self::Bgeu(b_type) => {}
            // LOAD
            Self::Lb(i_type) => {}
            Self::Lh(i_type) => {}
            Self::Lw(i_type) => {}
            Self::Lbu(i_type) => {}
            Self::Lhu(i_type) => {}
            // STORE
            Self::Sb(s_type) => {}
            Self::Sh(s_type) => {}
            Self::Sw(s_type) => {}
            // OP-IMM
            Self::Addi(rtype) => {}
            Self::Slti(rtype) => {}
            Self::Sltiu(rtype) => {}
            Self::Xori(rtype) => {}
            Self::Ori(rtype) => {}
            Self::Andi(rtype) => {}
            Self::Slli(rtype) => {}
            Self::Srli(rtype) => {}
            Self::Srai(rtype) => {}
            // OP
            // It includes both base integer instructions and M-extension instructions
            Self::Add(r_type) => {}
            Self::Sub(r_type) => {}
            Self::Sll(r_type) => {}
            Self::Slt(r_type) => {}
            Self::Sltu(r_type) => {}
            Self::Xor(r_type) => {}
            Self::Srl(r_type) => {}
            Self::Sra(r_type) => {}
            Self::Or(r_type) => {}
            Self::And(r_type) => {}
            Self::Mul(r_type) => {}
            Self::Mulh(r_type) => {}
            Self::Mulhsu(r_type) => {}
            Self::Mulhu(r_type) => {}
            Self::Div(r_type) => {}
            Self::Divu(r_type) => {}
            Self::Rem(r_type) => {}
            Self::Remu(r_type) => {}
            // SYSTEM
            Self::Fence(i_type) => {}
            Self::Ecall(i_type) => {}
            Self::Ebreak(i_type) => {}
        }
        uop
    }
}
