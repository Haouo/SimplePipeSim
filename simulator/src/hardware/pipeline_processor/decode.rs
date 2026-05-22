use std::fmt;

use crate::hardware::uop::{AluOpOneSelect, AluOpTwoSelect, AluOpTypes, PreDecodeMicroOp};
use crate::riscv::instruction::Instruction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DecodeError {
    IllegalInstruction { raw_inst: u32, pc: u32 },
    UnsupportedFence { pc: u32 },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IllegalInstruction { raw_inst, pc } => {
                write!(f, "Unknown instruction: {raw_inst:#08X} at PC: {pc:#x}")
            }
            Self::UnsupportedFence { pc } => {
                write!(f, "FENCE is not implemented at PC: {pc:#x}")
            }
        }
    }
}

pub(super) fn decode(op: &mut PreDecodeMicroOp, regs: &[u32; 32]) -> Result<(), DecodeError> {
    match op.inst {
        Instruction::Add(inst)
        | Instruction::Sub(inst)
        | Instruction::Sll(inst)
        | Instruction::Slt(inst)
        | Instruction::Sltu(inst)
        | Instruction::Xor(inst)
        | Instruction::Srl(inst)
        | Instruction::Sra(inst)
        | Instruction::Or(inst)
        | Instruction::And(inst)
        | Instruction::Mul(inst)
        | Instruction::Mulh(inst)
        | Instruction::Mulhu(inst)
        | Instruction::Mulhsu(inst)
        | Instruction::Div(inst)
        | Instruction::Divu(inst)
        | Instruction::Rem(inst)
        | Instruction::Remu(inst) => {
            op.rs1 = Some((inst.rs1(), regs[inst.rs1() as usize]));
            op.rs2 = Some((inst.rs2(), regs[inst.rs2() as usize]));
            op.rd_index = Some(inst.rd());
            op.alu_result_as_rd_dst_value = true;
            op.alu_op1_sel = AluOpOneSelect::RegRs1;
            op.alu_op2_sel = AluOpTwoSelect::RegRs2;
        }
        Instruction::Addi(inst)
        | Instruction::Slti(inst)
        | Instruction::Sltiu(inst)
        | Instruction::Xori(inst)
        | Instruction::Ori(inst)
        | Instruction::Andi(inst)
        | Instruction::Slli(inst)
        | Instruction::Srli(inst)
        | Instruction::Srai(inst) => {
            op.rs1 = Some((inst.rs1(), regs[inst.rs1() as usize]));
            op.immediate_signext = inst.imm_sign_ext();
            op.rd_index = Some(inst.rd());
            op.alu_op1_sel = AluOpOneSelect::RegRs1;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_result_as_rd_dst_value = true;
        }
        Instruction::Lb(inst)
        | Instruction::Lh(inst)
        | Instruction::Lw(inst)
        | Instruction::Lbu(inst)
        | Instruction::Lhu(inst) => {
            op.rs1 = Some((inst.rs1(), regs[inst.rs1() as usize]));
            op.immediate_signext = inst.imm_sign_ext();
            op.rd_index = Some(inst.rd());
            op.alu_op1_sel = AluOpOneSelect::RegRs1;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.is_mem = true;
        }
        Instruction::Sb(inst) | Instruction::Sh(inst) | Instruction::Sw(inst) => {
            op.rs1 = Some((inst.rs1(), regs[inst.rs1() as usize]));
            op.rs2 = Some((inst.rs2(), regs[inst.rs2() as usize]));
            op.immediate_signext = inst.sign_ext();
            op.alu_op1_sel = AluOpOneSelect::RegRs1;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.is_mem = true;
            op.is_store = true;
        }
        Instruction::Beq(inst)
        | Instruction::Bne(inst)
        | Instruction::Blt(inst)
        | Instruction::Bge(inst)
        | Instruction::Bltu(inst)
        | Instruction::Bgeu(inst) => {
            op.rs1 = Some((inst.rs1(), regs[inst.rs1() as usize]));
            op.rs2 = Some((inst.rs2(), regs[inst.rs2() as usize]));
            op.immediate_signext = inst.sign_ext();
            op.alu_op1_sel = AluOpOneSelect::CurrentPc;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.is_branch = true;
        }
        Instruction::Jal(inst) => {
            op.immediate_signext = inst.sign_ext();
            op.rd_index = Some(inst.rd());
            op.rd_write_value = Some(op.pc + 4);
            op.alu_op1_sel = AluOpOneSelect::CurrentPc;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.is_branch = true;
        }
        Instruction::Jalr(inst) => {
            op.rs1 = Some((inst.rs1(), regs[inst.rs1() as usize]));
            op.rd_index = Some(inst.rd());
            op.rd_write_value = Some(op.pc + 4);
            op.immediate_signext = inst.imm_sign_ext();
            op.alu_op1_sel = AluOpOneSelect::RegRs1;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.is_branch = true;
        }
        Instruction::Lui(inst) => {
            op.rd_index = Some(inst.rd());
            op.immediate_signext = inst.sign_ext();
            op.alu_op1_sel = AluOpOneSelect::Zero;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.alu_result_as_rd_dst_value = true;
        }
        Instruction::Auipc(inst) => {
            op.rd_index = Some(inst.rd());
            op.immediate_signext = inst.sign_ext();
            op.alu_op1_sel = AluOpOneSelect::CurrentPc;
            op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
            op.alu_op_type = AluOpTypes::Add;
            op.alu_result_as_rd_dst_value = true;
        }
        Instruction::Fence(_) => return Err(DecodeError::UnsupportedFence { pc: op.pc }),
        Instruction::Ecall(_) => op.is_env_call = true,
        Instruction::Illegal(raw_inst) => {
            return Err(DecodeError::IllegalInstruction {
                raw_inst,
                pc: op.pc,
            })
        }
    }

    if let Some(alu_op_type) = alu_op_type(op.inst) {
        op.alu_op_type = alu_op_type;
    }

    Ok(())
}

fn alu_op_type(inst: Instruction) -> Option<AluOpTypes> {
    match inst {
        Instruction::Add(_) | Instruction::Addi(_) => Some(AluOpTypes::Add),
        Instruction::Sub(_) => Some(AluOpTypes::Sub),
        Instruction::Sll(_) | Instruction::Slli(_) => Some(AluOpTypes::Sll),
        Instruction::Slt(_) | Instruction::Slti(_) => Some(AluOpTypes::Slt),
        Instruction::Sltu(_) | Instruction::Sltiu(_) => Some(AluOpTypes::Sltu),
        Instruction::Xor(_) | Instruction::Xori(_) => Some(AluOpTypes::Xor),
        Instruction::Srl(_) | Instruction::Srli(_) => Some(AluOpTypes::Srl),
        Instruction::Sra(_) | Instruction::Srai(_) => Some(AluOpTypes::Sra),
        Instruction::Or(_) | Instruction::Ori(_) => Some(AluOpTypes::Or),
        Instruction::And(_) | Instruction::Andi(_) => Some(AluOpTypes::And),
        Instruction::Mul(_) => Some(AluOpTypes::Mul),
        Instruction::Mulh(_) => Some(AluOpTypes::Mulh),
        Instruction::Mulhu(_) => Some(AluOpTypes::Mulhu),
        Instruction::Mulhsu(_) => Some(AluOpTypes::Mulhsu),
        Instruction::Div(_) => Some(AluOpTypes::Div),
        Instruction::Divu(_) => Some(AluOpTypes::Divu),
        Instruction::Rem(_) => Some(AluOpTypes::Rem),
        Instruction::Remu(_) => Some(AluOpTypes::Remu),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riscv::format_types::IType;

    #[test]
    fn op_imm_reads_register_and_selects_immediate_alu_operand() {
        let mut regs = [0; 32];
        regs[2] = 0xAA;
        let mut op = PreDecodeMicroOp {
            inst: Instruction::Addi(IType((2 << 15) | (3 << 7))),
            ..Default::default()
        };

        decode(&mut op, &regs).expect("decode addi");

        assert_eq!(op.rs1, Some((2, 0xAA)));
        assert_eq!(op.rd_index, Some(3));
        assert_eq!(op.alu_op_type, AluOpTypes::Add);
        assert_eq!(op.alu_op2_sel, AluOpTwoSelect::ImmSignExt);
    }
}
