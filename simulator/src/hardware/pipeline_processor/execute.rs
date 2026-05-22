use crate::hardware::uop::{AluOpOneSelect, AluOpTwoSelect, AluOpTypes, PreDecodeMicroOp};
use crate::riscv::instruction::Instruction;

const MUL_STALL: usize = 8;
const DIV_REM_STALL: usize = 32;

pub(super) fn m_extension_stall(inst: Instruction) -> Option<usize> {
    match inst {
        Instruction::Mul(_)
        | Instruction::Mulh(_)
        | Instruction::Mulhu(_)
        | Instruction::Mulhsu(_) => Some(MUL_STALL),
        Instruction::Div(_) | Instruction::Divu(_) | Instruction::Rem(_) | Instruction::Remu(_) => {
            Some(DIV_REM_STALL)
        }
        _ => None,
    }
}

pub(super) fn alu_operands(op: &PreDecodeMicroOp) -> (u32, u32) {
    let op1 = match op.alu_op1_sel {
        AluOpOneSelect::RegRs1 => op.rs1.expect("rs1 of current inst. should not be None!").1,
        AluOpOneSelect::CurrentPc => op.pc,
        AluOpOneSelect::Zero => 0,
    };
    let op2 = match op.alu_op2_sel {
        AluOpTwoSelect::RegRs2 => op.rs2.expect("rs2 of current inst. should not be None!").1,
        AluOpTwoSelect::ImmSignExt => op.immediate_signext,
    };
    (op1, op2)
}

pub(super) fn alu_result(op_type: AluOpTypes, op1: u32, op2: u32) -> u32 {
    match op_type {
        AluOpTypes::Add => op1.wrapping_add(op2),
        AluOpTypes::Sub => op1.wrapping_sub(op2),
        AluOpTypes::Sll => op1 << (op2 & 0x1f),
        AluOpTypes::Slt => u32::from((op1 as i32) < (op2 as i32)),
        AluOpTypes::Sltu => u32::from(op1 < op2),
        AluOpTypes::Xor => op1 ^ op2,
        AluOpTypes::Srl => op1 >> (op2 & 0x1f),
        AluOpTypes::Sra => ((op1 as i32) >> (op2 & 0x1f)) as u32,
        AluOpTypes::Or => op1 | op2,
        AluOpTypes::And => op1 & op2,
        AluOpTypes::Mul => (((op1 as i32) as i64) * ((op2 as i32) as i64)) as u32,
        AluOpTypes::Mulh => ((((op1 as i32) as i64) * ((op2 as i32) as i64)) >> 32) as u32,
        AluOpTypes::Mulhu => ((op1 as u64) * (op2 as u64) >> 32) as u32,
        AluOpTypes::Mulhsu => (((op1 as i32) as i64) * (op2 as i64) >> 32) as u32,
        AluOpTypes::Div => {
            if op2 != 0 {
                (((op1 as i32) as i64) / ((op2 as i32) as i64)) as u32
            } else {
                0xffff_ffff
            }
        }
        AluOpTypes::Divu => {
            if op2 != 0 {
                ((op1 as u64) / (op2 as u64)) as u32
            } else {
                0xffff_ffff
            }
        }
        AluOpTypes::Rem => {
            if op2 != 0 {
                ((op1 as i32).wrapping_rem(op2 as i32)) as u32
            } else {
                op1
            }
        }
        AluOpTypes::Remu => {
            if op2 != 0 {
                op1 % op2
            } else {
                op1
            }
        }
    }
}

pub(super) fn branch_taken(inst: Instruction, rs1: u32, rs2: u32) -> Option<bool> {
    match inst {
        Instruction::Jal(_) | Instruction::Jalr(_) => Some(true),
        Instruction::Beq(_) => Some(rs1 == rs2),
        Instruction::Bne(_) => Some(rs1 != rs2),
        Instruction::Blt(_) => Some((rs1 as i32) < (rs2 as i32)),
        Instruction::Bge(_) => Some((rs1 as i32) >= (rs2 as i32)),
        Instruction::Bltu(_) => Some(rs1 < rs2),
        Instruction::Bgeu(_) => Some(rs1 >= rs2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riscv::format_types::BType;

    #[test]
    fn rv32_division_zero_cases_match_architecture() {
        assert_eq!(alu_result(AluOpTypes::Div, 12, 0), u32::MAX);
        assert_eq!(alu_result(AluOpTypes::Rem, 12, 0), 12);
    }

    #[test]
    fn signed_and_unsigned_branches_compare_different_domains() {
        let inst = BType(0);
        assert_eq!(
            branch_taken(Instruction::Blt(inst), u32::MAX, 1),
            Some(true)
        );
        assert_eq!(
            branch_taken(Instruction::Bltu(inst), u32::MAX, 1),
            Some(false)
        );
    }
}
