use crate::riscv::instruction::Instruction;

/// The data type representing a Index-Value-Pair for general purpose registers in RISC-V ISA.
/// This can represent rs1, rs2 or rd. It can also be None, bacause not all instructions have rs1, rs2 or rd.
///
/// Due to characteristics of pipeline processor that the destination values is not always ready.
/// Hence, the second fields of the tuple wrapped by Option<...> is also a Option (Option<u32>), and it can be None
/// if the destination value (the value to be written-back) is not ready.
type RegisterIndexValuePair = Option<(u8, Option<u32>)>;

pub enum WriteBackSelect {
    LoadData,
    PcPlus4,
    AluOut,
}

impl Default for WriteBackSelect {
    fn default() -> Self {
        Self::AluOut
    }
}

#[derive(Default)]
pub struct PreDecodeMicroOp {
    /// general information
    pub inst: Instruction,
    pub pc: u32,

    /// register sources information
    pub rs1: RegisterIndexValuePair,
    pub rs2: RegisterIndexValuePair,
    pub immediate_signext: u32,
    pub shamt: u8,

    // EXE stage output
    pub alu_result: Option<u32>,

    // register destination information
    pub rd: RegisterIndexValuePair,

    // whether it is memory access instruction
    pub is_mem: bool,
    pub is_store: bool,
    pub mem_access_addr: Option<u32>,
    // `mem_value` stores the load value,
    // or the value need to be stored into memory
    pub mem_load_value: Option<u32>,

    // write-back selection
    pub wb_sel: WriteBackSelect,

    // branch information
    pub is_branch: bool,
    pub branch_taken: bool,
    pub target_address: Option<u32>,
}
