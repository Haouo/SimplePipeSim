#[derive(Default)]
pub struct UOp {
    /// general information
    pub raw_inst: u32,
    pub pc: u32,

    /// register sources information
    pub rs1: Option<(u8, u32)>,
    pub rs2: Option<(u8, u32)>,
    pub immediate_signext: u32,
    pub shamt: u8,

    // EXE stage output
    pub exe_out: u32,
    pub branch_unit_out: u32,

    // register destination information
    pub rd: Option<(u8, u32)>,
    pub is_rd_value_ready: bool, // it will be useful for data hazard detection

    // whether it is memory access instruction
    pub is_mem: bool,
    pub is_store: bool,
    pub access_addr: u32,
    // `mem_value` stores the load value,
    // or the value need to be stored into memory
    pub mem_value: u32,

    // branch information
    pub is_branch: bool,
    pub branch_taken: bool,
    pub target_address: Option<u32>,
}
