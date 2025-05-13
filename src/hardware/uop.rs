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
    Mulu,
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
pub enum WriteBackSelect {
    LoadData,
    PcPlus4,
    AluOut,
    #[default]
    WriteBackDisable,
}

#[derive(Default)]
pub struct PreDecodeMicroOp {
    // basic info
    pub inst: Instruction, // generated in IF
    pub pc: u32,           // generated in IF
    pub opcode: u8,        // generated in IF

    // register sources information
    pub rs1: Option<(u8, u32)>, // might be generated in ID
    pub rs2: Option<(u8, u32)>, // might be generated in ID
    pub immediate_signext: u32, // might be generated in ID

    // EXE stage output
    pub alu_op_type: AluOpTypes,     // generated in ID
    pub alu_op1_sel: AluOpOneSelect, // generated in ID
    pub alu_op2_sel: AluOpTwoSelect, // generated in ID
    pub alu_result: u32,             // generated in EXE

    // register destination information
    // The index of rd is generated in ID,
    pub rd_index: u8,

    // whether it is memory access instruction
    pub is_mem: bool,                 // generated in ID
    pub is_store: bool,               // generated in ID
    pub mem_access_addr: Option<u32>, // calculated in EXE
    // `mem_value` stores the load value,
    // or the value need to be stored into memory
    pub mem_load_value: Option<u32>, // generated in MEM

    // write-back selection
    pub wb_sel: WriteBackSelect, // generated in ID

    // branch information
    pub is_branch: bool,     // generated in ID
    pub branch_taken: bool,  // determined in ID or EXE, depends on whether it's conditional branch
    pub target_address: u32, // calculated in EXE as well

    // is environment call
    pub is_env_call: bool, // generated in ID
}
