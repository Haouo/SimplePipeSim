use crate::riscv::instruction::Instruction;

use super::clock::Clocked;
use super::mem::abstract_mem::*;
use super::mem::general_cache::replacement_policy::fifo;
use super::mem::general_cache::GeneralCache;
use super::mem::simple_mem::SimpleMem;
use super::uop::*;

use std::cell::{Cell, RefCell};
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
    id_op: Option<PreDecodeMicroOp>,
    exe_op: Option<PreDecodeMicroOp>,
    mem_op: Option<PreDecodeMicroOp>,
    wb_op: Option<PreDecodeMicroOp>,

    // waiting information about memory related stages (ID and MEM)
    if_is_wating: Option<MemoryReqType>,
    mem_is_waiting: Option<MemoryReqType>,

    // information for branch misprediction
    branch_recovery: bool,
    branch_flushes: u8,

    // imitate stall for integer mul/div instructions
    int_mul_div_stall: u8,

    // L1 Instruction Cache
    icache: Box<dyn AbstraceMemInterface>,
    // l1 data cache
    dcache: Box<dyn AbstraceMemInterface>,
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
            if_is_wating: None,
            mem_is_waiting: None,
            branch_recovery: false,
            branch_flushes: 0,
            int_mul_div_stall: 0,
            // todo: modify the cache parameters
            icache: Box::new(GeneralCache::<fifo::FifoRP>::new(
                4096,
                4,
                32,
                Rc::clone(&mem_ref),
            )),
            dcache: Box::new(GeneralCache::<fifo::FifoRP>::new(
                4096,
                4,
                32,
                Rc::clone(&mem_ref),
            )),
        }
    }
    /// ### Instruction Fetch Pipeline Stage Function
    fn pipe_stage_fetch(&mut self) {
        // stall current stage, because downstream is stalled
        if self.id_op.is_some() {
            return;
        }

        // handle inflight instruction fetching operation
        if self.if_is_wating.is_some() {
            // check whether the inst. fetching memory request is done
            if !self.if_is_wating.as_ref().unwrap().get_done() {
                return;
            }

            // memory load request is done
            let if_mem_ref = self.if_is_wating.take().unwrap();
            let load_data: [u8; 4] = if_mem_ref.get_load_req_ref().buffer.borrow()[0..=3]
                .try_into()
                .expect("The length of load data in IF stage should be 4.");
            let inst_raw_binary = u32::from_le_bytes(load_data);
            // put Instruction information into id_op and return
            self.id_op.as_mut().unwrap().inst = Instruction::raw_binary_to_inst(inst_raw_binary);
            return;
        }

        // handle new instruction fetching operation
        let new_mem_load_req = MemoryReqType::Load(MemoryLoadReq {
            addr: self.if_pc, // use IF-stage PC as fetching base address
            len: 4,           // no compressed inst.
            done: Rc::new(Cell::new(false)),
            buffer: Rc::new(RefCell::new(Box::from([0u8; 4]))),
        });
        let icache_register_result = self.icache.try_register_req(&new_mem_load_req);
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
        // stall
        if self.mem_op.is_none() {
            return;
        }

        // handling ongoing load/store instruction
        if self.mem_is_waiting.is_some() {
            // check whether the current memory request is ready
            if !self.mem_is_waiting.as_ref().unwrap().get_done() {
                return;
            }

            // if the memory request has been done
            //

            return;
        }

        // handle new load/store instruction
        let current_op = self.mem_op.take().unwrap();
    }

    /// ### Architectural Register File Write-back Pipeline Stage Function
    fn pipe_stage_wb(&mut self) {
        // stall
        if self.wb_op.is_none() {
            return;
        }

        let current_op = self.wb_op.take().unwrap();
        if current_op.rd.is_some() {
            let rd_index = current_op.rd.as_ref().unwrap().0;
            // need to write-back to register file
            if rd_index != 0 {
                self.id_regs[rd_index as usize] = current_op.rd.as_ref().unwrap().1.unwrap();
            }
        }
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
}
