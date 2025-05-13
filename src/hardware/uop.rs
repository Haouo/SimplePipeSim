use crate::riscv::instruction::Instruction;

/// The data type representing a Index-Value-Pair for general purpose registers in RISC-V ISA.
/// This can represent rs1, rs2 or rd. It can also be None, because not all instructions have rs1, rs2 or rd.
///
/// Due to characteristics of pipeline processor that the destination values is not always ready.
/// Hence, the second fields of the tuple wrapped by Option<...> is also a Option (Option<u32>), and it can be None
/// if the destination value (the value to be written-back) is not ready.
type RegisterIndexValuePair = Option<(u8, Option<u32>)>;

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
}

#[derive(Default)]
pub enum WriteBackSelect {
    LoadData,
    PcPlus4,
    AluOut,
    #[default]
    WriteDiable,
}

#[derive(Default)]
pub struct PreDecodeMicroOp {
    // basic info
    pub inst: Instruction, // generated in IF
    pub pc: u32,           // generated in IF
    pub opcode: u8,        // generated in IF

    // register sources information
    pub rs1: RegisterIndexValuePair, // might be generated in ID
    pub rs2: RegisterIndexValuePair, // might be generated in ID
    pub immediate_signext: u32,      // might be generated in ID

    // EXE stage output
    pub alu_op_type: AluOpTypes, // generated in ID
    pub alu_result: u32,         // generated in EXE

    // register destination information
    // The index of rd is generated in ID,
    // while the time of generation of the actual value of rd
    // depends on the instructin and pipeline itself.
    pub rd: RegisterIndexValuePair,

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
}
