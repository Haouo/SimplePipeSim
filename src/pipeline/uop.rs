#[derive(Default)]
pub struct IndexValuePair {
    pub idx: u8,
    pub value: u32,
}

#[derive(Default)]
pub struct UOp {
    /// general information
    pub raw_inst: u32,
    pub pc: u32,

    /// register sources information
    pub rs1: Option<IndexValuePair>,
    pub rs2: Option<IndexValuePair>,
    pub immediate_signext: u32,
    pub shamt: u8,

    /// register destination information
    pub rd: Option<IndexValuePair>,
    pub is_rd_value_ready: bool, // it will be useful for data hazard detection

    /// whether it is memory access instruction
    pub is_mem: bool,
    pub is_store: bool,
    pub access_addr: u32,
    /// `mem_value` stores the load value,
    /// or the value need to be stored into memory
    pub mem_value: u32,

    /// branch information
    pub is_branch: bool,
    pub branch_taken: bool,
    pub target_address: Option<u32>,
}
