use crate::pipeline::clock::Clocked;
use crate::pipeline::mem::general_cache::replacement_policy as rp;
use crate::pipeline::mem::general_cache::GeneralCache;
use crate::pipeline::mem::simple_mem::SimpleMem;
use crate::pipeline::uop::UOp;

use std::cell::RefCell;
use std::rc::Rc;

/// # Public struct `PipeState`
///
/// This struct contains the necessary information to imitate a classic 5 stage RISC-V pipeline processor
pub struct FiveStagePipeStage {
    // IF-Stage instruction fetch PC
    if_pc: u32,

    // register file (be accessed in ID and WB)
    id_regs: [u32; 32],

    // ID, EXE, MEM, WB micro-op
    // If the micro-op is Option::None, it means that
    // the current pipeline stage must be stalled.
    id_op: Option<UOp>,
    exe_op: Option<UOp>,
    mem_op: Option<UOp>,
    wb_op: Option<UOp>,

    // whether the memory request is completed
    mem_done: Option<bool>,

    // information for branch misprediction
    branch_recovery: bool,
    branch_flushes: u8,

    // imitate stall for integer mul/div instructions
    int_mul_div_stall: u8,

    // L1 Instruction Cache
    icache: GeneralCache<rp::random::RandomRP>,
    // l1 data cache
    dcache: GeneralCache<rp::random::RandomRP>,
}

impl FiveStagePipeStage {
    /// The constructor of PipeState struct.
    ///
    /// This function also have the responsibility for initialization the object.
    pub fn new(init_pc: u32, mem_ref: Rc<RefCell<SimpleMem>>) -> Self {
        FiveStagePipeStage {
            if_pc: init_pc,
            id_regs: [0; 32],
            id_op: None,
            exe_op: None,
            mem_op: None,
            wb_op: None,
            mem_done: None,
            branch_recovery: false,
            branch_flushes: 0,
            int_mul_div_stall: 0,
            // todo: modify the cache parameters
            icache: GeneralCache::new(0, 0, 0, Rc::clone(&mem_ref)),
            dcache: GeneralCache::new(0, 0, 0, Rc::clone(&mem_ref)),
        }
    }
    /// ### Instruction Fetch Pipeline Stage Function
    fn pipe_stage_fetch(&mut self) {
        todo!();
    }
    /// ### Instruction Decode Pipeline Stage Function
    fn pipe_stage_decode(&mut self) {
        todo!();
    }
    /// ### Instruction Execute Pipeline Stage Function
    fn pipe_stage_exe(&mut self) {
        todo!();
    }
    /// ### Memory Access Pipeline Stage Function
    fn pipe_stage_mem(&mut self) {
        todo!();
    }
    /// ### Architectural Register File Write-back Pipeline Stage Function
    fn pipe_stage_wb(&mut self) {
        todo!();
    }
    /// ### Branch Recovering Stage when meeting branch mis-prediction
    fn pipe_branch_recover(&mut self) {
        todo!();
    }
}

impl Clocked for FiveStagePipeStage {
    /// ### tick() function to simulate clock-edge trigger
    ///
    /// We should consider the simulation order of pipeline stage carefully.
    /// The main reason is about data hazard and data forwarding.
    fn tick(&mut self) {
        self.pipe_stage_wb();
        self.pipe_stage_mem();
        self.pipe_stage_exe();
        self.pipe_stage_decode();
        self.pipe_stage_fetch();
        self.pipe_branch_recover();
    }
    // **Ps**
    // One possible optimization for better performance:
    // the multi-threaded execution for these pipeline stages might be a solution
    // while it requires carefully using of Mutex and other synchronization tools
    //
    // another reason to do multi-threading is that is it more like the nature of the real
    // hardware, which concurrently execute
}
