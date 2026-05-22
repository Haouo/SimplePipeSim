use crate::riscv::encoding::OpcodeMap;
use crate::riscv::instruction::Instruction;

use super::super::branch_predictor::{BranchPredict, BranchPredictResult};
use super::super::clock::Clocked;
use super::super::mem::abstract_mem::*;
use super::super::mem::general_cache::replacement_policy::ReplacementPolicy;
use super::super::mem::general_cache::GeneralCache;
use super::super::statistic::Statistic;
use super::super::uop::*;
use super::memory_access::{MemoryTransaction, MemoryTransactionStatus};
use super::statistic;
use super::{decode, execute};
use crate::hardware::mem::general_cache::GeneralCacheConfig;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
enum PipelineMemoryFSM {
    #[default]
    Idle,
    SendingReq(MemoryReqType),
    WaitingComplete(MemoryReqType),
}

/// Public struct `PipelineProcessor`
///
/// This struct contains the necessary information to imitate a classic 5 stage RISC-V pipeline processor.
pub struct PipelineProcessor<RP, M>
where
    RP: ReplacementPolicy,
    M: AbstractMemoryInterface,
{
    // Halt signal
    pub halt: bool,

    // IF-Stage instruction fetch PC
    if_pc: u32, // composition of PC with its Branch Predict Result
    if_raw_isnt_buffer: Option<u32>,
    branch_predictor: Box<dyn BranchPredict>,

    // register file (be accessed in ID and WB)
    id_regs: [u32; 32],

    // ID, EXE, MEM, WB micro-op
    // If the micro-op is Option::None, it means that
    // the current pipeline stage must be stalled.
    id_op: Option<PreDecodeMicroOp>,
    exe_op: Option<PreDecodeMicroOp>,
    mem_op: Option<PreDecodeMicroOp>,
    wb_op: Option<PreDecodeMicroOp>,

    // FSM control logics about instruction fetch and data memory access.
    if_fsm: PipelineMemoryFSM,
    mem_transaction: MemoryTransaction,

    // information for branch misprediction
    branch_recover: bool,
    branch_correct_direction: bool,
    branch_destination: u32,
    branch_flushes: usize,

    // imitate stall for integer mul/div instructions
    int_mul_div_stall_countdown: Option<usize>,

    // L1 Instruction Cache
    pub icache: GeneralCache<RP, GeneralCache<RP, M>>,
    // L1 Data Cache
    pub dcache: GeneralCache<RP, GeneralCache<RP, M>>,
    // L2 Unified Cache (shared by icache and dcache)
    pub l2_cache: Rc<RefCell<GeneralCache<RP, M>>>,

    // statistic information (also called Hardware Performance Monitor, HPM)
    pub hpm: statistic::StatisticInfo,
}

impl<RP, M> PipelineProcessor<RP, M>
where
    RP: ReplacementPolicy,
    M: AbstractMemoryInterface,
{
    /// The constructor of PipeState struct.
    ///
    /// This function also have the responsibility for initialization the object.
    pub fn new(
        init_pc: u32,
        l1i_cache_config: GeneralCacheConfig,
        l1d_cache_config: GeneralCacheConfig,
        l2_cache_config: GeneralCacheConfig,
        mem_ref: &Rc<RefCell<M>>,
    ) -> Self {
        Self::new_with_predictor(
            init_pc,
            l1i_cache_config,
            l1d_cache_config,
            l2_cache_config,
            mem_ref,
            Box::new(super::super::branch_predictor::bimodal::Predictor::new()),
        )
    }

    pub fn new_with_predictor(
        init_pc: u32,
        l1i_cache_config: GeneralCacheConfig,
        l1d_cache_config: GeneralCacheConfig,
        l2_cache_config: GeneralCacheConfig,
        mem_ref: &Rc<RefCell<M>>,
        branch_predictor: Box<dyn BranchPredict>,
    ) -> Self {
        let l2_cache = Rc::new(RefCell::new(GeneralCache::<RP, M>::new(
            l2_cache_config,
            Rc::clone(mem_ref),
        )));
        PipelineProcessor {
            halt: false,
            if_pc: init_pc,
            if_raw_isnt_buffer: None,
            branch_predictor,
            id_regs: [0; 32],
            id_op: None,
            exe_op: None,
            mem_op: None,
            wb_op: None,
            if_fsm: PipelineMemoryFSM::default(),
            mem_transaction: MemoryTransaction::default(),
            branch_recover: false,
            branch_correct_direction: false,
            branch_destination: 0,
            branch_flushes: 0,
            int_mul_div_stall_countdown: None,
            icache: GeneralCache::<RP, GeneralCache<RP, M>>::new(
                l1i_cache_config,
                l2_cache.clone(),
            ),
            dcache: GeneralCache::<RP, GeneralCache<RP, M>>::new(
                l1d_cache_config,
                l2_cache.clone(),
            ),
            l2_cache,
            hpm: statistic::StatisticInfo::default(),
        }
    }

    pub fn registers(&self) -> [u32; 32] {
        self.id_regs
    }

    /// ### Instruction Fetch Pipeline Stage Function
    fn pipe_stage_fetch(&mut self) {
        match self.if_fsm {
            PipelineMemoryFSM::Idle => {
                let new_req = MemoryReqType::load(self.if_pc, 4);
                if self.icache.try_register_req(&new_req).is_err() {
                    self.if_fsm = PipelineMemoryFSM::SendingReq(new_req);
                    return;
                }

                // succesfully register new request to icache
                // check whether it is hit in the current cycle
                if new_req.is_done() == false {
                    self.if_fsm = PipelineMemoryFSM::WaitingComplete(new_req);
                    return;
                }

                // get data in the current cycle
                let load_bytes = new_req.load_data();
                let load_data_arr: [u8; 4] = load_bytes[0..=3]
                    .try_into()
                    .expect("The length of fetched inst. in IF Stage should be 4!");
                let load_data = u32::from_le_bytes(load_data_arr);
                self.if_raw_isnt_buffer = Some(load_data);
            }
            PipelineMemoryFSM::SendingReq(ref new_req) => {
                if self.icache.try_register_req(new_req).is_err() {
                    return;
                }
                if new_req.is_done() == false {
                    // need to wait for icache
                    self.if_fsm = PipelineMemoryFSM::WaitingComplete(new_req.clone());
                    return;
                }

                // get data in the current cycle
                let load_bytes = new_req.load_data();
                let load_data_arr: [u8; 4] = load_bytes[0..=3]
                    .try_into()
                    .expect("The length of fetched inst. in IF Stage should be 4!");
                let load_data = u32::from_le_bytes(load_data_arr);
                self.if_raw_isnt_buffer = Some(load_data);
            }
            PipelineMemoryFSM::WaitingComplete(ref inflight_req) => {
                if inflight_req.is_done() == false {
                    return;
                }

                // get data in the current cycle
                let load_bytes = inflight_req.load_data();
                let load_data_arr: [u8; 4] = load_bytes[0..=3]
                    .try_into()
                    .expect("The length of fetched inst. in IF Stage should be 4!");
                let load_data = u32::from_le_bytes(load_data_arr);
                self.if_raw_isnt_buffer = Some(load_data);
            }
        }

        // propagate uOp __only when__ downstream __is not__ stalled
        if self.id_op.is_some() == false {
            // small pre-decoding logic for checking control-flow inst.
            let raw_inst = self
                .if_raw_isnt_buffer
                .expect("The instruction buffer should not be None at the point!");
            let opcode: OpcodeMap = ((raw_inst & 0x7f) as u8)
                .try_into()
                .or(Result::<OpcodeMap, ()>::Ok(OpcodeMap::Op)) // dummy value for unknown OPCODE
                .unwrap();
            let new_raw_inst = self
                .if_raw_isnt_buffer
                .take()
                .expect("Getched data in if_raw_inst_buffer should not be None!");
            let new_inst = Instruction::raw_binary_to_inst(new_raw_inst);
            self.hpm.inst_fetch(); // update HPM

            // tranfer raw binary data to Instructrion and make PreDecodeMicroOp
            self.id_op = if let OpcodeMap::Branch | OpcodeMap::Jal | OpcodeMap::Jalr = opcode {
                // make branch prediction for next cycle **only when** new_inst is control-flow inst.
                let next_cycle_bp_result: BranchPredictResult =
                    self.branch_predictor.branch_predict(self.if_pc);
                let old_pc = self.if_pc;

                // update PC
                if next_cycle_bp_result.direction {
                    self.if_pc = next_cycle_bp_result.addr;
                } else {
                    self.if_pc += 4;
                }

                // make new uOp to ID
                Some(PreDecodeMicroOp::fetched_branch(
                    new_raw_inst,
                    new_inst,
                    old_pc,
                    next_cycle_bp_result,
                ))
            } else {
                let old_pc = self.if_pc;
                // update PC
                self.if_pc += 4;

                // make new uOp to ID
                Some(PreDecodeMicroOp::fetched(new_raw_inst, new_inst, old_pc))
            };

            // reset if_fsm
            self.if_fsm = PipelineMemoryFSM::Idle;
        } // if downstream is stalled while the fetched data is ready, just pass current cycle and caches the fetch data
    }

    /// ### Instruction Decode Pipeline Stage Function
    ///
    /// In the ID-stage, it pre-decodes the instruction from IF-stage into PreDecodeMicroOp with necessary information.
    /// IT can help the latter stages to perform specific operations without complex decoding logic.
    fn pipe_stage_decode(&mut self, additional_stall: bool) {
        // stall current stage if downstream stage are stalled
        if self.exe_op.is_some() {
            return;
        }
        // stall if `self.id_op` is not ready (means that ID stage is still waiting for IF stage to produce new uOp)
        if self.id_op.is_none() {
            return;
        }
        // stall if there is Load-Use Hazard which needs additional stall even with data forwarding
        if additional_stall {
            return;
        }

        if let Some(ref mut current_op) = self.id_op {
            decode::decode(current_op, &self.id_regs).unwrap_or_else(|error| panic!("{error}"));
        }

        // transfer id_op to exe_op
        self.exe_op = self.id_op.take();
    }

    /// ### Instruction Execute Pipeline Stage Function
    fn pipe_stage_exe(&mut self) {
        // decrease M-ext. stall counter even if the downstream is stalled
        if let Some(ref mut cnt) = self.int_mul_div_stall_countdown {
            if *cnt > 0 {
                *cnt -= 1;
            }
        }

        // stall EXE stage if downstream is stalled
        if self.mem_op.is_some() {
            return;
        }
        // stall EXE stage if ID stage did not complete his job in last cycle
        if self.exe_op.is_none() {
            return;
        }

        /* Assume that all possible RAW data hazards have been solved at this point. */

        // handle current inst. at EXE stage
        if let Some(ref mut current_op) = self.exe_op {
            if let Instruction::Ecall(_) = current_op.inst {
                self.mem_op = self.exe_op.take();
                return;
            }
            if let Some(stall_cycles) = execute::m_extension_stall(current_op.inst) {
                if self.int_mul_div_stall_countdown.is_none() {
                    self.int_mul_div_stall_countdown = Some(stall_cycles);
                    return;
                }
            }

            let (op1, op2) = execute::alu_operands(current_op);
            current_op.alu_result = execute::alu_result(current_op.alu_op_type, op1, op2);
            if execute::m_extension_stall(current_op.inst).is_some() {
                self.int_mul_div_stall_countdown = None;
            }

            // whether current ALU Result is write-back data to destination register rd
            if current_op.alu_result_as_rd_dst_value {
                current_op.set_writeback_value(current_op.alu_result);
            }

            // Resolve control-flow instructions (conditional/unconditional branches)
            //
            // It must judge whether the last branch prediction result is correct and
            // notify the pipeline to perform branch recovery in need.
            if let Some(branch) = current_op.branch_resolution_inputs() {
                let is_taken = execute::branch_taken(branch.inst, branch.rs1, branch.rs2)
                    .expect("Non-control-flow instructions should not reach branch resolution");

                // check whether the last branch prediction is incorrect
                // There are two possible incorrec results
                // 1. Predicted direction is wrong
                // 2. Predicted direction is matched (both taken) while the predicted target PC is wrong
                let need_recover = branch.needs_recovery(is_taken);
                if need_recover {
                    self.branch_recover = true;
                    self.branch_correct_direction = is_taken;
                    self.branch_destination = branch.recovery_destination(is_taken);
                    self.branch_flushes = 3;
                }
                self.hpm.solve_branch(need_recover);
            }
        }

        // transfer exe_op to mem_op
        self.mem_op = self.exe_op.take();
    }

    /// ### Memory Access Pipeline Stage Function
    fn pipe_stage_mem(&mut self) {
        // stall if there is no any job to do
        if self.mem_op.is_none() {
            return;
        }

        let load_value = if let Some(access) = self.mem_op.as_ref().unwrap().memory_access() {
            match self.mem_transaction.progress(&mut self.dcache, access) {
                MemoryTransactionStatus::Pending => return,
                MemoryTransactionStatus::Complete { load_value } => load_value,
            }
        } else {
            None
        };

        if let Some(load_value) = load_value {
            self.mem_op
                .as_mut()
                .expect("MEM-stage op should exist while materializing a load")
                .set_writeback_value(load_value);
        }

        // stall if downstream is stalled
        if self.wb_op.is_some() {
            return;
        }

        self.mem_transaction.reset();

        // transfer mem_op to wb_op
        self.wb_op = self.mem_op.take();
    }

    /// ### Architectural Register File Write-back Pipeline Stage Function
    fn pipe_stage_wb(&mut self) {
        // stall if there is o any job to do
        if self.wb_op.is_none() {
            return;
        }

        // handle SYSTEM inst. (e.g. ECALL)
        if let Instruction::Ecall(_) = self.wb_op.as_ref().unwrap().inst {
            let reg_a0 = self.id_regs[10];
            let reg_a1 = self.id_regs[11];
            if reg_a0 == 0 {
                self.halt = true; // ends the simulation
            } else if reg_a0 == 1 {
                // print!("{}", char::from_u32(reg_a1 & 0xff).unwrap()); // print a character which is stored in $a1
            }
        }

        // handle current instruction in WB stage
        let current_op = self.wb_op.take().unwrap(); // take() consumes wb_op
        if let Some((rd_index, rd_write_value)) = current_op.writeback() {
            if rd_index != 0 {
                self.id_regs[rd_index as usize] = rd_write_value;
            }
        }
        if current_op.is_placeholder() == false {
            self.hpm.inst_ret(); // update HPM
        }
    }

    /// Try to solve RAW hazards before the start of a new cycle by using data forwarding
    fn pipe_normal_data_hazard_resolve(&mut self) {
        // ---------- Inportant Note -------- //
        // The information of rs1 and rs2 of the instruction in ID stage is not present at
        // the point when this function `pipe_data_forwarding()` is called.
        //
        // Hence, it has to extract the rs1 and rs2 information to detect the Load-use hazard
        // instead of using the `self.id_op.rs1` and `self.id_op.rs2` directly.
        // ---------------------------------- //

        // Check scenario 1: WB -> EXE Forwarding Path (lower priority)
        if let (Some(exe_op), Some(wb_op)) = (&mut self.exe_op, &self.wb_op) {
            // check exe_op.rs1 <---> wb_op.rd
            if let (Some(exe_rs1), Some((wb_rd_idx, wb_rd_write_value))) =
                (&mut exe_op.rs1, wb_op.writeback())
            {
                if (exe_rs1.0 != 0) && (exe_rs1.0 == wb_rd_idx) {
                    exe_rs1.1 = wb_rd_write_value;
                }
            }
            // check exe_op.rs2 <---> wb_op.rd
            if let (Some(exe_rs2), Some((wb_rd_idx, wb_rd_write_value))) =
                (&mut exe_op.rs2, wb_op.writeback())
            {
                if (exe_rs2.0 != 0) && (exe_rs2.0 == wb_rd_idx) {
                    exe_rs2.1 = wb_rd_write_value;
                }
            }
        }

        // Check scenario 2: MEM -> EXE Forwarding Path (higher proiority)
        // it might override the forwarding data of scenario 1
        // because the latter instruction (in MEM stage) has newest data
        if let (Some(exe_op), Some(mem_op)) = (&mut self.exe_op, &self.mem_op) {
            // check exe_op.rs1 <---> mem_op.rd
            if let (Some(exe_rs1), Some((mem_rd_idx, mem_rd_write_value))) =
                (&mut exe_op.rs1, mem_op.writeback())
            {
                if (exe_rs1.0 != 0) && (exe_rs1.0 == mem_rd_idx) {
                    exe_rs1.1 = mem_rd_write_value;
                }
            }
            // check exe_op.rs2 <---> mem_op.rd
            if let (Some(exe_rs2), Some((mem_rd_idx, mem_rd_write_value))) =
                (&mut exe_op.rs2, mem_op.writeback())
            {
                if (exe_rs2.0 != 0) && (exe_rs2.0 == mem_rd_idx) {
                    exe_rs2.1 = mem_rd_write_value;
                }
            }
        }
    }

    fn pipe_load_use_hazard_handle(&mut self) -> bool {
        // Checks whether the original instruction at EXE stage has moved to MEM stage
        if let (Some(id_op), None, Some(as_exe_op)) = (&self.id_op, &self.exe_op, &self.mem_op) {
            // check the original EXE instruction is LOAD
            if as_exe_op.is_load()
                && as_exe_op
                    .destination_register()
                    .is_some_and(|destination| destination != 0)
            {
                let id_opcode: OpcodeMap = ((id_op.raw_inst & 0x7f) as u8)
                    .try_into()
                    .expect("Illegal OPCODE!");
                match id_opcode {
                    // these 5 types do not use rs1 and rs2 source registers
                    OpcodeMap::Jal
                    | OpcodeMap::Lui
                    | OpcodeMap::Auipc
                    | OpcodeMap::MiscMem
                    | OpcodeMap::System => {}
                    // other types use rs1 and rs2
                    _ => {
                        let id_rs1 = ((id_op.raw_inst >> 15) & 0x1f) as u8;
                        let id_rs2 = ((id_op.raw_inst >> 20) & 0x1f) as u8;
                        if id_rs1 != 0 && Some(id_rs1) == as_exe_op.destination_register() {
                            return true;
                        }
                        if id_rs2 != 0 && Some(id_rs2) == as_exe_op.destination_register() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

impl<RP, M> Clocked for PipelineProcessor<RP, M>
where
    RP: ReplacementPolicy,
    M: AbstractMemoryInterface,
{
    /// ### tick() function to simulate clock-edge trigger
    ///
    /// We should consider the simulation order of pipeline stage carefully.
    /// The main reason is about data hazard and data forwarding.
    fn tick(&mut self) {
        self.hpm.tick();
        // println!("IF PC: {:#x}", self.if_pc);
        // println!(
        //     "IF | {} | {} | {} | {}",
        //     self.id_op.is_some(),
        //     self.exe_op.is_some(),
        //     self.mem_op.is_some(),
        //     self.wb_op.is_some()
        // );

        // handle branch recovery for branch miss-prediction
        if self.branch_recover {
            // revocer correct if_pc and clean fetched inst. or inflight request
            self.if_pc = self.branch_destination;
            self.if_fsm = PipelineMemoryFSM::default();
            self.if_raw_isnt_buffer = None;

            // flush pipeline stages
            if self.branch_flushes >= 2 {
                self.hpm.inst_flush(self.id_op.is_some());
                self.id_op = None;
            }
            if self.branch_flushes >= 3 {
                self.hpm.inst_flush(self.exe_op.is_some());
                self.exe_op = None;
            }
            if self.branch_flushes >= 4 {
                self.hpm.inst_flush(self.mem_op.is_some());
                self.mem_op = None;
                self.mem_transaction.reset();
            }
            if self.branch_flushes >= 5 {
                self.hpm.inst_flush(self.wb_op.is_some());
                self.wb_op = None;
            }

            // update Branch Predictor
            self.branch_predictor.mispredict_recovery(
                self.branch_correct_direction,
                self.mem_op.as_ref().unwrap().pc, // the inst. caused control-flow changing in now in MEM stage
                self.branch_destination,
            );

            // clean self.branch_recover flag
            self.branch_recover = false;
        }

        // start new simulation cycle
        self.pipe_normal_data_hazard_resolve();
        self.pipe_stage_wb();
        self.pipe_stage_mem();
        self.pipe_stage_exe();
        let id_load_use_stall = self.pipe_load_use_hazard_handle();
        self.pipe_stage_decode(id_load_use_stall);
        self.pipe_stage_fetch();
        // insert additional NOP instruction at EXE stage to prevent from
        // the instruction at ID stage propagating to EXE stage
        //
        // Inserting NOP makes the self.exe_op to be Some(...) instead of None
        if id_load_use_stall {
            assert!(self.exe_op.is_none());
            self.exe_op = Some(PreDecodeMicroOp::placeholder_nop());
        }

        // tick L1-I$, L1-D$, and L2$
        self.icache.tick();
        self.dcache.tick();
        self.l2_cache.borrow_mut().tick();
    }
}

impl<RP, M> Statistic for PipelineProcessor<RP, M>
where
    RP: ReplacementPolicy,
    M: AbstractMemoryInterface,
{
    type StatisticInfo = statistic::StatisticInfo;
    fn get_statistic_info(&self) -> Self::StatisticInfo {
        // calculate parts of statistics info.
        let mut ret = self.hpm.clone();
        ret.finalize_rates();
        // return
        ret
    }
}
