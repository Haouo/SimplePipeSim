use crate::riscv::encoding::OpcodeMap;
use crate::riscv::instruction::Instruction;

use super::super::branch_predictor::{BranchPredict, BranchPredictResult};
use super::super::clock::Clocked;
use super::super::mem::abstract_mem::*;
use super::super::mem::general_cache::replacement_policy::ReplacementPolicy;
use super::super::mem::general_cache::GeneralCache;
use super::super::mem::simple_mem::SimpleMem;
use super::super::statistic::Statistic;
use super::super::uop::*;
use super::statistic::StatisticInfo;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Additional stalls for integer MUL instructions
const MUL_STALL: usize = 8;
/// Additional stalls for integer DIV/REM instructions
const DIV_REM_STALL: usize = 32;

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
pub struct PipelineProcessor<RP>
where
    RP: ReplacementPolicy,
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

    // FSM control logics about memory related stages (ID and MEM)
    if_fsm: PipelineMemoryFSM,
    mem_fsm: PipelineMemoryFSM,
    mem_access_length_buffer: Option<usize>,
    mem_load_buffer: Option<u32>,

    // information for branch misprediction
    branch_recover: bool,
    branch_correct_direction: bool,
    branch_destination: u32,
    branch_flushes: usize,

    // imitate stall for integer mul/div instructions
    int_mul_div_stall_countdown: Option<usize>,

    // L1 Instruction Cache
    // icache: Box<dyn AbstractMemoryInterface>,
    icache: GeneralCache<RP>,
    // l1 data cache
    // dcache: Box<dyn AbstractMemoryInterface>,
    dcache: GeneralCache<RP>,

    // statistic information (also called Hardware Performance Monitor, HPM)
    pub hpm: StatisticInfo,
}

impl<RP> PipelineProcessor<RP>
where
    RP: ReplacementPolicy,
{
    /// The constructor of PipeState struct.
    ///
    /// This function also have the responsibility for initialization the object.
    pub fn new(init_pc: u32, mem_ref: &Rc<RefCell<SimpleMem>>) -> Self {
        PipelineProcessor {
            halt: false,
            if_pc: init_pc,
            if_raw_isnt_buffer: None,
            branch_predictor: Box::new(super::super::branch_predictor::bimodal::Predictor::new()),
            id_regs: [0; 32],
            id_op: None,
            exe_op: None,
            mem_op: None,
            wb_op: None,
            if_fsm: PipelineMemoryFSM::default(),
            mem_fsm: PipelineMemoryFSM::default(),
            mem_access_length_buffer: None,
            mem_load_buffer: None,
            branch_recover: false,
            branch_correct_direction: false,
            branch_destination: 0,
            branch_flushes: 0,
            int_mul_div_stall_countdown: None,
            // I$ configuration: 4096 bytes in total, 4-way associativity, 32 bytes for each block (implies 32 sets)
            icache: GeneralCache::<RP>::new("L1-I$".to_string(), 4096, 4, 16, Rc::clone(mem_ref)),
            dcache: GeneralCache::<RP>::new("L1-D$".to_string(), 4096, 4, 16, Rc::clone(mem_ref)),
            hpm: StatisticInfo::default(),
        }
    }

    /// ### Instruction Fetch Pipeline Stage Function
    fn pipe_stage_fetch(&mut self) {
        match self.if_fsm {
            PipelineMemoryFSM::Idle => {
                let new_req = MemoryReqType::Load(MemoryLoadReq {
                    addr: self.if_pc,
                    len: 4,
                    done: Rc::new(Cell::new(false)),
                    buffer: Rc::new(RefCell::from(vec![0u8; 4].into_boxed_slice())),
                });
                if self.icache.try_register_req(&new_req).is_err() {
                    self.if_fsm = PipelineMemoryFSM::SendingReq(new_req);
                    return;
                }

                // succesfully register new request to icache
                // check whether it is hit in the current cycle
                if new_req.get_done() == false {
                    self.if_fsm = PipelineMemoryFSM::WaitingComplete(new_req);
                    return;
                }

                // get data in the current cycle
                let load_data_arr: [u8; 4] = new_req.get_load_req_ref().buffer.borrow()[0..=3]
                    .try_into()
                    .expect("The length of fetched inst. in IF Stage should be 4!");
                let load_data = u32::from_le_bytes(load_data_arr);
                self.if_raw_isnt_buffer = Some(load_data);
            }
            PipelineMemoryFSM::SendingReq(ref new_req) => {
                if self.icache.try_register_req(new_req).is_err() {
                    return;
                }
                if new_req.get_done() == false {
                    // need to wait for icache
                    self.if_fsm = PipelineMemoryFSM::WaitingComplete(new_req.clone());
                    return;
                }

                // get data in the current cycle
                let load_data_arr: [u8; 4] = new_req.get_load_req_ref().buffer.borrow()[0..=3]
                    .try_into()
                    .expect("The length of fetched inst. in IF Stage should be 4!");
                let load_data = u32::from_le_bytes(load_data_arr);
                self.if_raw_isnt_buffer = Some(load_data);
            }
            PipelineMemoryFSM::WaitingComplete(ref inflight_req) => {
                if inflight_req.get_done() == false {
                    return;
                }

                // get data in the current cycle
                let load_data_arr: [u8; 4] = inflight_req.get_load_req_ref().buffer.borrow()[0..=3]
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
                Some(PreDecodeMicroOp {
                    raw_inst: new_raw_inst,
                    inst: new_inst,
                    pc: old_pc,
                    is_branch: true,
                    bp_result: Some(next_cycle_bp_result),
                    ..Default::default()
                })
            } else {
                let old_pc = self.if_pc;
                // update PC
                self.if_pc += 4;

                // make new uOp to ID
                Some(PreDecodeMicroOp {
                    raw_inst: new_raw_inst,
                    inst: new_inst,
                    pc: old_pc,
                    is_branch: false,
                    ..Default::default()
                })
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

        // Stage 1 pre-decoding
        // generate most of isnt. info. and read data from register file
        if let Some(ref mut current_op) = self.id_op {
            match current_op.inst {
                // OP
                Instruction::Add(inst)
                | Instruction::Sub(inst)
                | Instruction::Sll(inst)
                | Instruction::Slt(inst)
                | Instruction::Sltu(inst)
                | Instruction::Xor(inst)
                | Instruction::Srl(inst)
                | Instruction::Sra(inst)
                | Instruction::Or(inst)
                | Instruction::And(inst)
                | Instruction::Mul(inst)
                | Instruction::Mulh(inst)
                | Instruction::Mulhu(inst)
                | Instruction::Mulhsu(inst)
                | Instruction::Div(inst)
                | Instruction::Divu(inst)
                | Instruction::Rem(inst)
                | Instruction::Remu(inst) => {
                    current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize])); // read regfile
                    current_op.rs2 = Some((inst.rs2(), self.id_regs[inst.rs2() as usize])); // read regfile
                    current_op.rd_index = Some(inst.rd());
                    current_op.alu_result_as_rd_dst_value = true;
                    current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                    current_op.alu_op2_sel = AluOpTwoSelect::RegRs2;
                }

                // OP-IMM
                Instruction::Addi(inst)
                | Instruction::Slti(inst)
                | Instruction::Sltiu(inst)
                | Instruction::Xori(inst)
                | Instruction::Ori(inst)
                | Instruction::Andi(inst)
                | Instruction::Slli(inst)
                | Instruction::Srli(inst)
                | Instruction::Srai(inst) => {
                    current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize])); // read regfile
                    current_op.immediate_signext = inst.imm_sign_ext();
                    current_op.rd_index = Some(inst.rd());
                    current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_result_as_rd_dst_value = true;
                }

                // LOAD
                Instruction::Lb(inst)
                | Instruction::Lh(inst)
                | Instruction::Lw(inst)
                | Instruction::Lbu(inst)
                | Instruction::Lhu(inst) => {
                    current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize])); // read regfile
                    current_op.immediate_signext = inst.imm_sign_ext();
                    current_op.rd_index = Some(inst.rd());
                    current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add; // (rs1 + imm) as memory access addr.
                    current_op.is_mem = true;
                }

                // STORE
                Instruction::Sb(inst) | Instruction::Sh(inst) | Instruction::Sw(inst) => {
                    current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                    current_op.rs2 = Some((inst.rs2(), self.id_regs[inst.rs2() as usize])); // rs2 -> mem[rs1 + imm]
                    current_op.immediate_signext = inst.sign_ext();
                    current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add; // (rs1 + imm) as memory access addr.
                    current_op.is_mem = true;
                    current_op.is_store = true;
                }

                // Conditional BRANCH
                Instruction::Beq(inst)
                | Instruction::Bne(inst)
                | Instruction::Blt(inst)
                | Instruction::Bge(inst)
                | Instruction::Bltu(inst)
                | Instruction::Bgeu(inst) => {
                    current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                    current_op.rs2 = Some((inst.rs2(), self.id_regs[inst.rs2() as usize]));
                    current_op.immediate_signext = inst.sign_ext();
                    current_op.alu_op1_sel = AluOpOneSelect::CurrentPc;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add;
                    current_op.is_branch = true;
                }

                // JAL
                Instruction::Jal(inst) => {
                    current_op.immediate_signext = inst.sign_ext();
                    current_op.rd_index = Some(inst.rd());
                    current_op.rd_write_value = Some(current_op.pc + 4); // jump and "link"
                    current_op.alu_op1_sel = AluOpOneSelect::CurrentPc;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add;
                    current_op.is_branch = true;
                }
                // JALR
                Instruction::Jalr(inst) => {
                    current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                    current_op.rd_index = Some(inst.rd());
                    current_op.rd_write_value = Some(current_op.pc + 4);
                    current_op.immediate_signext = inst.imm_sign_ext(); // jump and "link"
                    current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add;
                    current_op.is_branch = true;
                }

                // LUI
                Instruction::Lui(inst) => {
                    current_op.rd_index = Some(inst.rd());
                    current_op.immediate_signext = inst.sign_ext();
                    current_op.alu_op1_sel = AluOpOneSelect::Zero;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add;
                    current_op.alu_result_as_rd_dst_value = true;
                }

                // AUIPC
                Instruction::Auipc(inst) => {
                    current_op.rd_index = Some(inst.rd());
                    current_op.immediate_signext = inst.sign_ext();
                    current_op.alu_op1_sel = AluOpOneSelect::CurrentPc;
                    current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                    current_op.alu_op_type = AluOpTypes::Add;
                    current_op.alu_result_as_rd_dst_value = true;
                }

                // FENCE
                Instruction::Fence(_inst) => {
                    todo!();
                }

                // SYSTEM
                Instruction::Ecall(_inst) => {
                    current_op.is_env_call = true;
                }

                // We capture illegal instructions in the ID stage and make the program panic.
                Instruction::Illegal(raw_inst) => panic!(
                    "Unknown instruction: {:#08X} at PC: {:#x}",
                    raw_inst, current_op.pc
                ),
            }

            // stage 2 pre-decode for OP and OP-IMM OPCODE types
            // It works mainly on decideing the ALU Operation types
            match current_op.inst {
                // Addition
                Instruction::Add(_) | Instruction::Addi(_) => {
                    current_op.alu_op_type = AluOpTypes::Add
                }
                // Subtraction
                Instruction::Sub(_) => current_op.alu_op_type = AluOpTypes::Sub,
                // Shift Left Logically
                Instruction::Sll(_) | Instruction::Slli(_) => {
                    current_op.alu_op_type = AluOpTypes::Sll
                }
                // Set on Less-than
                Instruction::Slt(_) | Instruction::Slti(_) => {
                    current_op.alu_op_type = AluOpTypes::Slt
                }
                // Set on Less-than Unsigned
                Instruction::Sltu(_) | Instruction::Sltiu(_) => {
                    current_op.alu_op_type = AluOpTypes::Sltu
                }
                // Bitwise XOR
                Instruction::Xor(_) | Instruction::Xori(_) => {
                    current_op.alu_op_type = AluOpTypes::Xor
                }
                // Shift Right Logically
                Instruction::Srl(_) | Instruction::Srli(_) => {
                    current_op.alu_op_type = AluOpTypes::Srl
                }
                // Shift Right Arithmetically
                Instruction::Sra(_) | Instruction::Srai(_) => {
                    current_op.alu_op_type = AluOpTypes::Sra
                }
                // Bitwise OR
                Instruction::Or(_) | Instruction::Ori(_) => current_op.alu_op_type = AluOpTypes::Or,
                // Bitwise AND
                Instruction::And(_) | Instruction::Andi(_) => {
                    current_op.alu_op_type = AluOpTypes::And
                }
                // Mul (signed, lower half part)
                Instruction::Mul(_) => current_op.alu_op_type = AluOpTypes::Mul,
                // Mul (signed, higher half part)
                Instruction::Mulh(_) => current_op.alu_op_type = AluOpTypes::Mulh,
                // Mulhu (unsigned, higher hal part)
                Instruction::Mulhu(_) => current_op.alu_op_type = AluOpTypes::Mulhu,
                // Mulhsu (signed * unsigned, higher half part)
                Instruction::Mulhsu(_) => current_op.alu_op_type = AluOpTypes::Mulhsu,
                // Div
                Instruction::Div(_) => current_op.alu_op_type = AluOpTypes::Div,
                // Div Unsigned
                Instruction::Divu(_) => current_op.alu_op_type = AluOpTypes::Divu,
                // Modulo
                Instruction::Rem(_) => current_op.alu_op_type = AluOpTypes::Rem,
                // Modulo Unsigned
                Instruction::Remu(_) => current_op.alu_op_type = AluOpTypes::Remu,
                _ => {}
            }
        }

        // transfer id_op to exe_op
        self.exe_op = self.id_op.take();

        // **Ps**
        // In hardware implementation, stage 1 and 2 can be implemented in parallel.
        // In other words, the hardware decoder can have two parallel decoding path because they are independent logically.
        // The first performs common decoding logics, and the second performs specific decoding logic for determining ALU OP-Types.
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
            // special actions for M-extension inst.
            match current_op.inst {
                Instruction::Mul(_)
                | Instruction::Mulh(_)
                | Instruction::Mulhu(_)
                | Instruction::Mulhsu(_) => {
                    if self.int_mul_div_stall_countdown.is_none() {
                        self.int_mul_div_stall_countdown = Some(MUL_STALL);
                        return;
                    }
                }
                Instruction::Div(_)
                | Instruction::Divu(_)
                | Instruction::Rem(_)
                | Instruction::Remu(_) => {
                    if self.int_mul_div_stall_countdown.is_none() {
                        self.int_mul_div_stall_countdown = Some(DIV_REM_STALL);
                        return;
                    }
                }
                _ => {}
            }

            // preset ALU's op-1 and op-2
            let op1: u32 = match current_op.alu_op1_sel {
                AluOpOneSelect::RegRs1 => {
                    current_op
                        .rs1
                        .expect("rs1 of current inst. should not be None!")
                        .1
                }
                AluOpOneSelect::CurrentPc => current_op.pc,
                AluOpOneSelect::Zero => 0u32,
            };
            let op2: u32 = match current_op.alu_op2_sel {
                AluOpTwoSelect::RegRs2 => {
                    current_op
                        .rs2
                        .expect("rs2 of current inst. should not be None!")
                        .1
                }
                AluOpTwoSelect::ImmSignExt => current_op.immediate_signext,
            };
            // perform numerical calculations
            // !!! TODO: there are problems about signed/unsigned arithmetics !!!
            match current_op.alu_op_type {
                AluOpTypes::Add => current_op.alu_result = op1.wrapping_add(op2),
                AluOpTypes::Sub => current_op.alu_result = op1.wrapping_sub(op2),
                AluOpTypes::Sll => current_op.alu_result = op1 << (op2 & 0x1f),
                AluOpTypes::Slt => {
                    current_op.alu_result = if (op1 as i32) < (op2 as i32) {
                        1u32
                    } else {
                        0u32
                    }
                }
                AluOpTypes::Sltu => current_op.alu_result = if op1 < op2 { 1u32 } else { 0u32 },
                AluOpTypes::Xor => current_op.alu_result = op1 ^ op2,
                AluOpTypes::Srl => current_op.alu_result = op1 >> (op2 & 0x1f), // logically (fill zeros on the MSB side)
                AluOpTypes::Sra => current_op.alu_result = ((op1 as i32) >> (op2 & 0x1f)) as u32, // arithmetically (fill sign-bits on the MSB side)
                AluOpTypes::Or => current_op.alu_result = op1 | op2,
                AluOpTypes::And => current_op.alu_result = op1 & op2,
                AluOpTypes::Mul => {
                    current_op.alu_result = (((op1 as i32) as i64) * ((op2 as i32) as i64)) as u32;
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Mulh => {
                    current_op.alu_result =
                        ((((op1 as i32) as i64) * ((op2 as i32) as i64)) >> 32) as u32;
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Mulhu => {
                    current_op.alu_result = ((op1 as u64) * (op2 as u64) >> 32) as u32;
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Mulhsu => {
                    current_op.alu_result = (((op1 as i32) as i64) * (op2 as i64) >> 32) as u32;
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Div => {
                    current_op.alu_result = if op2 != 0 {
                        (((op1 as i32) as i64) / ((op2 as i32) as i64)) as u32
                    } else {
                        0xffff_ffffu32
                    };
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Divu => {
                    current_op.alu_result = if op2 != 0 {
                        ((op1 as u64) / (op2 as u64)) as u32
                    } else {
                        0xffff_ffffu32
                    };
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Rem => {
                    current_op.alu_result = if op2 != 0 {
                        ((op1 as i32).wrapping_rem(op2 as i32)) as u32
                    } else {
                        op1
                    };
                    self.int_mul_div_stall_countdown = None;
                }
                AluOpTypes::Remu => {
                    current_op.alu_result = if op2 != 0 { op1 % op2 } else { op1 };
                    self.int_mul_div_stall_countdown = None;
                }
            }

            // whether current ALU Result is write-back data to destination register rd
            if current_op.alu_result_as_rd_dst_value {
                current_op.rd_write_value = Some(current_op.alu_result);
            }

            // Resolve control-flow instructions (conditional/unconditional branches)
            //
            // It must judge whether the last branch prediction result is correct and
            // notify the pipeline to perform branch recovery in need.
            if current_op.is_branch {
                #[allow(unused_assignments)]
                let mut is_taken = false;
                let branch_op1 = current_op.rs1.or(Some((0, 0))).unwrap().1;
                let branch_op2 = current_op.rs2.or(Some((0, 0))).unwrap().1;

                match current_op.inst {
                    // unconditional branch
                    Instruction::Jal(_) | Instruction::Jalr(_) => is_taken = true,
                    // conditional branch
                    Instruction::Beq(_) => {
                        is_taken = if branch_op1 == branch_op2 {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bne(_) => {
                        is_taken = if branch_op1 != branch_op2 {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Blt(_) => {
                        is_taken = if (branch_op1 as i32) < (branch_op2 as i32) {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bge(_) => {
                        is_taken = if (branch_op1 as i32) >= (branch_op2 as i32) {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bltu(_) => {
                        is_taken = if branch_op1 < branch_op2 { true } else { false }
                    }
                    Instruction::Bgeu(_) => {
                        is_taken = if branch_op1 >= branch_op2 {
                            true
                        } else {
                            false
                        }
                    }

                    // non-branch inst.
                    _ => {
                        unreachable!("Non-control-flow instructions should not get to here!");
                    }
                }

                // check whether the last branch prediction is incorrect
                // There are two possible incorrec results
                // 1. Predicted direction is wrong
                // 2. Predicted direction is matched (both taken) while the predicted target PC is wrong
                let need_recover = if current_op.bp_result.unwrap().direction != is_taken {
                    true // scenario 1
                } else {
                    if is_taken && current_op.bp_result.unwrap().addr != current_op.alu_result {
                        true // scenario 2
                    } else {
                        false
                    }
                };
                if need_recover {
                    self.branch_recover = true;
                    self.branch_correct_direction = is_taken;
                    self.branch_destination = if is_taken {
                        current_op.alu_result
                    } else {
                        current_op.pc + 4
                    };
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

        // handle the interaction with dcache
        match self.mem_fsm {
            PipelineMemoryFSM::Idle => {
                // issue new request to dcache
                if let Some(ref current_op) = self.mem_op {
                    if current_op.is_mem {
                        // first check access length
                        let access_length = match current_op.inst {
                            Instruction::Lw(_) | Instruction::Sw(_) => 4,
                            Instruction::Lh(_) | Instruction::Lhu(_) | Instruction::Sh(_) => 2,
                            Instruction::Lb(_) | Instruction::Lbu(_) | Instruction::Sb(_) => 1,
                            _ => unreachable!(),
                        };
                        self.mem_access_length_buffer = Some(access_length);

                        let new_req = if current_op.is_store {
                            MemoryReqType::Store(MemoryStoreReq {
                                addr: current_op.alu_result,
                                len: access_length,
                                store_data: Box::from(
                                    &current_op.rs2.unwrap().1.to_le_bytes()[0..access_length],
                                ),
                                done: Rc::new(Cell::new(false)),
                            })
                        } else {
                            MemoryReqType::Load(MemoryLoadReq {
                                addr: current_op.alu_result,
                                len: access_length,
                                done: Rc::new(Cell::new(false)),
                                buffer: Rc::new(RefCell::new(
                                    vec![0u8; access_length].into_boxed_slice(),
                                )),
                            })
                        };
                        if self.dcache.try_register_req(&new_req).is_err() {
                            self.mem_fsm = PipelineMemoryFSM::SendingReq(new_req);
                            return;
                        }

                        // register new request to dcache successfully
                        // check whether it is hit
                        if new_req.get_done() == false {
                            self.mem_fsm = PipelineMemoryFSM::WaitingComplete(new_req);
                            return;
                        }
                        // cache hit at the current cycle
                        if current_op.is_store == false {
                            // for load inst.
                            let mut tmp_vec =
                                vec![0u8; self.mem_access_length_buffer.take().unwrap()];
                            tmp_vec.clone_from_slice(&*new_req.get_load_req_ref().buffer.borrow());
                            let mut load_data = 0u32;
                            for (idx, a_byte) in tmp_vec.iter().enumerate() {
                                load_data += (*a_byte as u32) << (8 * idx);
                            }
                            self.mem_load_buffer = Some(load_data);
                        }
                    }
                }
            }
            PipelineMemoryFSM::SendingReq(ref new_req) => {
                if self.dcache.try_register_req(new_req).is_err() {
                    return;
                }

                // register new request to dcache successfully
                // check whether it is hit
                if new_req.get_done() == false {
                    self.mem_fsm = PipelineMemoryFSM::WaitingComplete(new_req.clone());
                    return;
                }
                // cache hit at the current cycle
                if self.mem_op.as_ref().unwrap().is_store == false {
                    // for load inst.
                    let mut tmp_vec = vec![0u8; self.mem_access_length_buffer.take().unwrap()];
                    tmp_vec.clone_from_slice(&*new_req.get_load_req_ref().buffer.borrow());
                    let mut load_data = 0u32;
                    for (idx, a_byte) in tmp_vec.iter().enumerate() {
                        load_data += (*a_byte as u32) << (8 * idx);
                    }
                    self.mem_load_buffer = Some(load_data);
                }
            }
            PipelineMemoryFSM::WaitingComplete(ref inflight_req) => {
                if inflight_req.get_done() == false {
                    return;
                }
                // cache hit at the current cycle
                if self.mem_op.as_ref().unwrap().is_store == false {
                    // for load inst.
                    let mut tmp_vec = vec![0u8; self.mem_access_length_buffer.take().unwrap()];
                    tmp_vec.clone_from_slice(&*inflight_req.get_load_req_ref().buffer.borrow());
                    let mut load_data = 0u32;
                    for (idx, a_byte) in tmp_vec.iter().enumerate() {
                        load_data += (*a_byte as u32) << (8 * idx);
                    }
                    self.mem_load_buffer = Some(load_data);
                }
            }
        }

        // stall if downstream is stalled
        if self.wb_op.is_some() {
            return;
        }

        // move load data from buffer to rd_write_value
        // need to handle signed or zero extension at here
        if let Some(load_data) = self.mem_load_buffer.take() {
            self.mem_op.as_mut().unwrap().rd_write_value = match self.mem_op.as_ref().unwrap().inst
            {
                Instruction::Lhu(_) | Instruction::Lbu(_) | Instruction::Lw(_) => Some(load_data),
                Instruction::Lh(_) => Some((((load_data << 16) as i32) >> 16) as u32),
                Instruction::Lb(_) => Some((((load_data << 24) as i32) >> 24) as u32),
                _ => unreachable!(),
            }
        }

        // reset mem_fsm
        self.mem_fsm = PipelineMemoryFSM::Idle;

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
                print!("{}", char::from_u32(reg_a1 & 0xff).unwrap()); // print a character which is stored in $a1
            }
        }

        // handle current instruction in WB stage
        let current_op = self.wb_op.take().unwrap(); // take() consumes wb_op
        if let Some(rd_index) = current_op.rd_index {
            if rd_index != 0 {
                self.id_regs[current_op.rd_index.unwrap() as usize] =
                    current_op.rd_write_value.unwrap();
            }
        }
        if current_op.placeholder == false {
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
            if let (Some(exe_rs1), Some(wb_rd_idx), Some(wb_rd_write_value)) =
                (&mut exe_op.rs1, wb_op.rd_index, wb_op.rd_write_value)
            {
                if (exe_rs1.0 != 0) && (exe_rs1.0 == wb_rd_idx) {
                    exe_rs1.1 = wb_rd_write_value;
                }
            }
            // check exe_op.rs2 <---> wb_op.rd
            if let (Some(exe_rs2), Some(wb_rd_idx), Some(wb_rd_write_value)) =
                (&mut exe_op.rs2, wb_op.rd_index, wb_op.rd_write_value)
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
            if let (Some(exe_rs1), Some(mem_rd_idx), Some(mem_rd_write_value)) =
                (&mut exe_op.rs1, mem_op.rd_index, mem_op.rd_write_value)
            {
                if (exe_rs1.0 != 0) && (exe_rs1.0 == mem_rd_idx) {
                    exe_rs1.1 = mem_rd_write_value;
                }
            }
            // check exe_op.rs2 <---> mem_op.rd
            if let (Some(exe_rs2), Some(mem_rd_idx), Some(mem_rd_write_value)) =
                (&mut exe_op.rs2, mem_op.rd_index, mem_op.rd_write_value)
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
            if as_exe_op.is_mem && !as_exe_op.is_store && as_exe_op.rd_index.is_some_and(|x| x != 0)
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
                        if id_rs1 != 0 && id_rs1 == as_exe_op.rd_index.unwrap() {
                            return true;
                        }
                        if id_rs2 != 0 && id_rs2 == as_exe_op.rd_index.unwrap() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

impl<RP> Clocked for PipelineProcessor<RP>
where
    RP: ReplacementPolicy,
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
                self.mem_fsm = PipelineMemoryFSM::default();
                self.mem_load_buffer = None;
                self.mem_access_length_buffer = None;
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
            self.exe_op = Some(PreDecodeMicroOp::generate_nop());
            self.exe_op.as_mut().unwrap().placeholder = true;
        }

        // tick L1-I$ and L1-D$
        self.icache.tick();
        self.dcache.tick();
    }
}

impl<RP> Statistic for PipelineProcessor<RP>
where
    RP: ReplacementPolicy,
{
    fn show_statistic_info(&self) {
        // info of two caches
        self.icache.show_statistic_info();
        self.dcache.show_statistic_info();
        // info of the CPU itself
        println!("=============================================");
        print!("{}", self.hpm);
        println!("=============================================");
        println!();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::hardware::mem::general_cache::replacement_policy as rp;
    use crate::hardware::mem::simple_mem::SimpleMem;
    use crate::sim::elf;

    use std::path::Path;

    #[test]
    fn riscv_tests() {
        let inst_list: Vec<&str> = vec![
            "rv32ui-p-add",
            "rv32ui-p-lui",
            "rv32ui-p-sub",
            "rv32ui-p-addi",
            "rv32ui-p-and",
            "rv32ui-p-andi",
            "rv32ui-p-lw",
            "rv32ui-p-lh",
            "rv32ui-p-lhu",
            "rv32ui-p-lb",
            "rv32ui-p-lbu",
            "rv32ui-p-sw",
            "rv32ui-p-sh",
            "rv32ui-p-sb",
            "rv32ui-p-beq",
            "rv32ui-p-bne",
            "rv32ui-p-blt",
            "rv32ui-p-bge",
            "rv32ui-p-bltu",
            "rv32ui-p-bgeu",
            "rv32ui-p-jal",
            "rv32ui-p-jalr",
            "rv32ui-p-or",
            "rv32ui-p-ori",
            "rv32ui-p-sll",
            "rv32ui-p-slli",
            "rv32ui-p-sra",
            "rv32ui-p-srai",
            "rv32ui-p-srl",
            "rv32ui-p-srli",
            "rv32ui-p-slt",
            "rv32ui-p-sltu",
            "rv32ui-p-slti",
            "rv32ui-p-sltiu",
            "rv32ui-p-auipc",
            "rv32um-p-mul",
            "rv32um-p-mulh",
            "rv32um-p-mulhu",
            "rv32um-p-mulhsu",
            "rv32um-p-div",
            "rv32um-p-divu",
            "rv32um-p-rem",
            "rv32um-p-remu",
        ];

        // run each unit test for each instructions
        let path_prefix = Path::new("../riscv-tests/isa");
        for inst_name in inst_list {
            let path = path_prefix.join(Path::new(inst_name));
            let elf::ProgramInfo {
                entry_pc,
                prog_body,
            } = elf::elf_loader(&path);
            let mem = Rc::new(RefCell::new(SimpleMem::new(prog_body)));
            let mut cpu = PipelineProcessor::<rp::fifo::FifoRP>::new(entry_pc, &mem);

            while cpu.halt == false {
                cpu.tick();
                mem.borrow_mut().tick();
            }

            assert_eq!(
                cpu.id_regs[3],
                1, // it pass the test if the value in $gp is 1
                "Does not pass test {}, the wrong TESTNUM is {}",
                inst_name,
                (cpu.id_regs[3] - 1) / 2
            );
            println!("Pass the test: {}", inst_name);
            cpu.show_statistic_info();
        }
    }

    #[test]
    fn general_programs() {
        let general_prog_names: Vec<&str> = vec!["hello", "print_nums", "msort", "qsort", "matmul"];
        let path_prefix = Path::new("../target/riscv32im-unknown-none-elf/debug");
        for prog_name in general_prog_names {
            let elf::ProgramInfo {
                entry_pc,
                prog_body,
            } = elf::elf_loader(&path_prefix.join(prog_name));

            let mem = Rc::new(RefCell::new(SimpleMem::new(prog_body)));
            let mut cpu = PipelineProcessor::<rp::fifo::FifoRP>::new(entry_pc, &mem);

            while cpu.halt == false {
                cpu.tick();
                mem.borrow_mut().tick();
            }
            cpu.show_statistic_info();
        }
    }
}
