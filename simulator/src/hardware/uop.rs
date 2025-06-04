use super::branch_predictor::BranchPredictResult;
use crate::riscv::format_types::IType;
use crate::riscv::instruction::Instruction;

#[derive(Default)]
pub enum AluOpTypes {
    #[default]
    Add,
    Sll,
    Slt,
    Sltu,
    Xor,
    Srl,
    Or,
    And,
    Sub,
    Sra,
    Mul,
    Mulh,
    Mulhu,
    Mulhsu,
    Div,
    Divu,
    Rem,
    Remu,
}

#[derive(Default)]
pub enum AluOpOneSelect {
    #[default]
    RegRs1,
    CurrentPc,
    Zero,
}

#[derive(Default)]
pub enum AluOpTwoSelect {
    #[default]
    RegRs2,
    ImmSignExt,
}

#[derive(Default)]
pub struct PreDecodeMicroOp {
    // basic info
    pub raw_inst: u32,     // generated in IF
    pub inst: Instruction, // generated in IF
    pub pc: u32,           // generated in IF

    // register sources information
    pub rs1: Option<(u8, u32)>, // might be generated in ID
    pub rs2: Option<(u8, u32)>, // might be generated in ID
    pub immediate_signext: u32, // might be generated in ID

    // EXE stage output
    pub alu_op_type: AluOpTypes,          // generated in ID
    pub alu_op1_sel: AluOpOneSelect,      // generated in ID
    pub alu_op2_sel: AluOpTwoSelect,      // generated in ID
    pub alu_result: u32, // generated in EXE (the pipeline might take many cycles to calculate)
    pub alu_result_as_rd_dst_value: bool, // whether the calculation result of ALU is write-value of rd register

    // register destination information
    // The index of rd is generated in ID,
    pub rd_index: Option<u8>,
    pub rd_write_value: Option<u32>, // might be generated in ID, EXE or MEM

    // whether it is memory access instruction
    pub is_mem: bool,   // generated in ID
    pub is_store: bool, // generated in ID

    // branch information
    pub is_branch: bool,                        // generated in IF
    pub bp_result: Option<BranchPredictResult>, // generated in IF

    // is environment call
    pub is_env_call: bool, // generated in ID

    // special flag: placeholder
    // The current uOp should be viewed as a normal instruction (NOP), which means that
    // it can take place of some pipeline stage and preventing from propagating of the instruction at the previous stage,
    // while the uOp with placeholder flags being true should not be counted into retired instructions.
    pub placeholder: bool,
}

impl PreDecodeMicroOp {
    pub fn generate_nop() -> Self {
        Self {
            pc: 0xffffffffu32,
            inst: Instruction::Addi(IType(0x00000013u32)),
            rs1: Some((0, 0)),
            rs2: Some((0, 0)),
            ..Default::default()
        }
    }
}
