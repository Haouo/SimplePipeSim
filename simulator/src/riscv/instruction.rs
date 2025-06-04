use super::encoding::OpcodeMap;
use super::format_types::*;

/// Enumeration of all supported instructions (RV32IM without Privileged Arch.)
#[derive(Clone, Copy, Debug)]
pub enum Instruction {
    //LUI
    Lui(UType),
    // AUIPC
    Auipc(UType),
    // JAL
    Jal(JType),
    // JALR
    Jalr(IType),

    // BRANCH
    Beq(BType),
    Bne(BType),
    Blt(BType),
    Bge(BType),
    Bltu(BType),
    Bgeu(BType),

    // LOAD
    Lb(IType),
    Lh(IType),
    Lw(IType),
    Lbu(IType),
    Lhu(IType),

    // STORE
    Sb(SType),
    Sh(SType),
    Sw(SType),

    // OP-IMM
    Addi(IType),
    Slti(IType),
    Sltiu(IType),
    Xori(IType),
    Ori(IType),
    Andi(IType),
    Slli(IType),
    Srli(IType),
    Srai(IType),

    // OP
    // It includes both base integer instructions and M-extension instructions
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

    // SYSTEM
    Fence(IType),
    Ecall(IType),

    // illega instruction identifier type
    Illegal(u32),
}

impl Default for Instruction {
    fn default() -> Self {
        Self::Illegal(0u32)
    }
}

impl Instruction {
    pub fn raw_binary_to_inst(raw_inst: u32) -> Self {
        let raw_opcode: u8 = (raw_inst & 0x7f) as u8;

        match OpcodeMap::try_from(raw_opcode) {
            Ok(opcode) => {
                match opcode {
                    OpcodeMap::Op => {
                        let inst = RType(raw_inst);
                        match inst.func3() {
                            0b000 => match inst.func7() {
                                0b0100000 => Self::Sub(inst),
                                0b0000000 => Self::Add(inst),
                                0b0000001 => Self::Mul(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b001 => match inst.func7() {
                                0 => Self::Sll(inst),
                                1 => Self::Mulh(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b010 => match inst.func7() {
                                0 => Self::Slt(inst),
                                1 => Self::Mulhsu(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b011 => match inst.func7() {
                                0 => Self::Sltu(inst),
                                1 => Self::Mulhu(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b100 => match inst.func7() {
                                0 => Self::Xor(inst),
                                1 => Self::Div(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b101 => match inst.func7() {
                                0 => Self::Srl(inst),
                                0b0100000 => Self::Sra(inst),
                                1 => Self::Divu(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b110 => match inst.func7() {
                                0 => Self::Or(inst),
                                1 => Self::Rem(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            0b111 => match inst.func7() {
                                0 => Self::And(inst),
                                1 => Self::Remu(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            _ => unreachable!(),
                        }
                    }

                    OpcodeMap::OpImm => {
                        let inst = IType(raw_inst);
                        match inst.func3() {
                            0b000 => Self::Addi(inst),
                            0b010 => Self::Slti(inst),
                            0b011 => Self::Sltiu(inst),
                            0b100 => Self::Xori(inst),
                            0b110 => Self::Ori(inst),
                            0b111 => Self::Andi(inst),
                            0b001 => Self::Slli(inst),
                            0b101 => match inst.imm11_0() >> 5 {
                                0 => Self::Srli(inst),
                                0b0100000 => Self::Srai(inst),
                                _ => Self::Illegal(raw_inst),
                            },
                            _ => unreachable!(),
                        }
                    }

                    // LOAD
                    OpcodeMap::Load => {
                        let inst = IType(raw_inst);
                        match inst.func3() {
                            0b000 => Self::Lb(inst),
                            0b001 => Self::Lh(inst),
                            0b010 => Self::Lw(inst),
                            0b100 => Self::Lbu(inst),
                            0b101 => Self::Lhu(inst),
                            _ => Self::Illegal(raw_inst),
                        }
                    }

                    // STORE
                    OpcodeMap::Store => {
                        let inst = SType(raw_inst);
                        match inst.func3() {
                            0b000 => Self::Sb(inst),
                            0b001 => Self::Sh(inst),
                            0b010 => Self::Sw(inst),
                            _ => Self::Illegal(raw_inst),
                        }
                    }

                    // BRANCH
                    OpcodeMap::Branch => {
                        let inst = BType(raw_inst);
                        match inst.func3() {
                            0b000 => Self::Beq(inst),
                            0b001 => Self::Bne(inst),
                            0b100 => Self::Blt(inst),
                            0b101 => Self::Bge(inst),
                            0b110 => Self::Bltu(inst),
                            0b111 => Self::Bgeu(inst),
                            _ => Self::Illegal(raw_inst),
                        }
                    }

                    OpcodeMap::Jal => Self::Jal(JType(raw_inst)),
                    OpcodeMap::Jalr => Self::Jalr(IType(raw_inst)),
                    OpcodeMap::Lui => Self::Lui(UType(raw_inst)),
                    OpcodeMap::Auipc => Self::Auipc(UType(raw_inst)),

                    // FENCE
                    OpcodeMap::MiscMem => {
                        let inst = IType(raw_inst);
                        todo!();
                    }

                    OpcodeMap::System => {
                        let inst = IType(raw_inst);
                        if let 0 = inst.imm11_0() {
                            Self::Ecall(inst)
                        } else {
                            Self::Illegal(raw_inst)
                        }
                    }
                }
            }
            Err(_) => Self::Illegal(raw_inst),
        }
    }
}
