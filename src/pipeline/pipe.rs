use crate::riscv::uop::UOp;

struct PipeState {
    // IF-Stage Inst. Fetch PC
    pc: u32,
    // register file (be accessed in ID and WB)
    regs: [u32; 32],
    // ID, EXE, MEM, WB micro-op
    id_op: Option<UOp>,
    exe_op: Option<UOp>,
    mem_op: Option<UOp>,
    wb_op: Option<UOp>,
    // information for branch misprediction
    branch_recovery: bool,
    branch_dest: bool,
    branch_flushes: u8,
    // mimcs stall for M-extension
    m_ext_stall: u8,
}

impl PipeState {
    pub fn new() -> Self {
        todo!();
    }
}
