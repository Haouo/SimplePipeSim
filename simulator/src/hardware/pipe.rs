use crate::riscv::encoding::OpcodeMap;
use crate::riscv::instruction::Instruction;

use super::branch_predictor::{BranchPredict, BranchPredictResult};
use super::clock::Clocked;
use super::mem::abstract_mem::*;
use super::mem::general_cache::replacement_policy::fifo;
use super::mem::general_cache::GeneralCache;
use super::mem::simple_mem::SimpleMem;
use super::uop::*;

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
pub struct PipelineProcessor {
    // Halt signal
    halt: bool,

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
    mem_load_buffer: Option<u32>,

    // information for branch misprediction
    branch_recover: bool,
    branch_correct_direction: bool,
    branch_destination: u32,
    branch_flushes: usize,

    // imitate stall for integer mul/div instructions
    int_mul_div_stall_countdown: Option<usize>,

    // L1 Instruction Cache
    icache: Box<dyn AbstractMemoryInterface>,
    // l1 data cache
    dcache: Box<dyn AbstractMemoryInterface>,
}

impl PipelineProcessor {
    /// The constructor of PipeState struct.
    ///
    /// This function also have the responsibility for initialization the object.
    pub fn new(init_pc: u32, mem_ref: Rc<RefCell<SimpleMem>>) -> Self {
        PipelineProcessor {
            halt: false,
            if_pc: init_pc,
            if_raw_isnt_buffer: None,
            branch_predictor: Box::new(super::branch_predictor::dummy::Predictor),
            id_regs: [0; 32],
            id_op: None,
            exe_op: None,
            mem_op: None,
            wb_op: None,
            if_fsm: PipelineMemoryFSM::default(),
            mem_fsm: PipelineMemoryFSM::default(),
            mem_load_buffer: None,
            branch_recover: false,
            branch_correct_direction: false,
            branch_destination: 0,
            branch_flushes: 0,
            int_mul_div_stall_countdown: None,
            // I$ configuration: 4096 bytes in total, 4-way associativity, 32 bytes for each block (implies 32 sets)
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

        let downstream_stalled = self.id_op.is_some()
            | self.exe_op.is_some()
            | self.mem_op.is_some()
            | self.wb_op.is_some();

        // println!(
        //     "ID {} EXE {} MEM {} WB {}",
        //     self.id_op.is_some(),
        //     self.exe_op.is_some(),
        //     self.mem_op.is_some(),
        //     self.wb_op.is_some()
        // );
        // propagate uOp __only when__ downstream __is not__ stalled
        if downstream_stalled == false {
            // small pre-decoding logic for checking control-flow inst.
            let raw_inst = self
                .if_raw_isnt_buffer
                .expect("The instruction buffer should not be None at the point!");
            let opcode: OpcodeMap = ((raw_inst & 0x7f) as u8)
                .try_into()
                .or(Result::<OpcodeMap, ()>::Ok(OpcodeMap::Op)) // dummy value for unknown OPCODE
                .unwrap();
            let new_inst = Instruction::raw_binary_to_inst(
                self.if_raw_isnt_buffer
                    .take()
                    .expect("Fetced data should not be None!"),
            );

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
        if self.exe_op.is_some() | self.mem_op.is_some() | self.wb_op.is_some() {
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
                Instruction::Illegal(raw_inst) => panic!("Unknown instruction: {:#08X}", raw_inst),
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
        if self.mem_op.is_some() | self.wb_op.is_some() {
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
                AluOpTypes::Add => current_op.alu_result = (op1 as i32 + op2 as i32) as u32,
                AluOpTypes::Sub => current_op.alu_result = (op1 as i32 - op2 as i32) as u32,
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
                        (((op1 as i32) as i64) / ((op1 as i32) as i64)) as u32
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
                        ((op1 as i32) % (op2 as i32)) as u32
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
                match current_op.inst {
                    // unconditional branch
                    Instruction::Jal(_) | Instruction::Jalr(_) => is_taken = true,
                    // conditional branch
                    Instruction::Beq(_) => is_taken = if op1 == op2 { true } else { false },
                    Instruction::Bne(_) => is_taken = if op1 != op2 { true } else { false },
                    Instruction::Blt(_) => {
                        is_taken = if (op1 as i32) < (op2 as i32) {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bge(_) => {
                        is_taken = if (op1 as i32) < (op2 as i32) {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bltu(_) => is_taken = if op1 < op2 { true } else { false },
                    Instruction::Bgeu(_) => is_taken = if op1 >= op2 { true } else { false },

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
                    self.branch_destination = current_op.alu_result;
                    self.branch_flushes = 3;
                }
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

                        let new_req = if current_op.is_store {
                            MemoryReqType::Store(MemoryStoreReq {
                                addr: current_op.alu_result,
                                len: access_length,
                                store_data: Box::from(current_op.rs2.unwrap().1.to_le_bytes()),
                                done: Rc::new(Cell::new(false)),
                            })
                        } else {
                            MemoryReqType::Load(MemoryLoadReq {
                                addr: current_op.alu_result,
                                len: access_length,
                                done: Rc::new(Cell::new(false)),
                                buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
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
                            let load_data_arr: [u8; 4] = new_req.get_load_req_ref().buffer.borrow()
                                [0..=3]
                                .try_into()
                                .unwrap();
                            self.mem_load_buffer = Some(u32::from_le_bytes(load_data_arr));
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
                    let load_data_arr: [u8; 4] = new_req.get_load_req_ref().buffer.borrow()[0..=3]
                        .try_into()
                        .unwrap();
                    self.mem_load_buffer = Some(u32::from_le_bytes(load_data_arr));
                }
            }
            PipelineMemoryFSM::WaitingComplete(ref inflight_req) => {
                if inflight_req.get_done() == false {
                    return;
                }
                // cache hit at the current cycle
                if self.mem_op.as_ref().unwrap().is_store == false {
                    // for load inst.
                    let load_data_arr: [u8; 4] = inflight_req.get_load_req_ref().buffer.borrow()
                        [0..=3]
                        .try_into()
                        .unwrap();
                    self.mem_load_buffer = Some(u32::from_le_bytes(load_data_arr));
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
    }

    /// Try to solve RAW hazards before the start of a new cycle by using data forwarding
    fn pipe_data_forwarding(&mut self) -> bool {
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

        // check Load-Use Data Hazard
        // firstly, check inst. at MEM stage is a load inst.
        if self
            .exe_op
            .as_ref()
            .is_some_and(|x| x.is_mem && (!x.is_store))
        {
            if self.id_op.is_some() {
                let exe_rd_index = self
                    .exe_op
                    .as_ref()
                    .unwrap()
                    .rd_index
                    .expect("Load instruction without rd index makes no sense.");
                // if ID rs1 and Load inst. at EXE rd is overlap
                if self
                    .id_op
                    .as_ref()
                    .unwrap()
                    .rs1
                    .is_some_and(|x| x.0 == exe_rd_index)
                {
                    return true;
                }
                // if ID rs2 and Load inst. at EXE rd is overlap
                if self
                    .id_op
                    .as_ref()
                    .unwrap()
                    .rs2
                    .is_some_and(|x| x.0 == exe_rd_index)
                {
                    return true;
                }
            }
        }
        false
    }
}

impl Clocked for PipelineProcessor {
    /// ### tick() function to simulate clock-edge trigger
    ///
    /// We should consider the simulation order of pipeline stage carefully.
    /// The main reason is about data hazard and data forwarding.
    fn tick(&mut self) {
        // println!("IF PC: {:#X}", self.if_pc);

        // handle branch recovery for branch miss-prediction
        if self.branch_recover {
            // revocer correct if_pc and clean fetched inst. or inflight request
            self.if_pc = self.branch_destination;
            self.if_fsm = PipelineMemoryFSM::default();
            self.if_raw_isnt_buffer = None;

            // flush pipeline stages
            if self.branch_flushes >= 2 {
                self.id_op = None;
            }
            if self.branch_flushes >= 3 {
                self.exe_op = None;
            }
            if self.branch_flushes >= 4 {
                self.mem_op = None;
                self.mem_fsm = PipelineMemoryFSM::default();
                self.mem_load_buffer = None;
            }
            if self.branch_flushes >= 5 {
                self.wb_op = None;
            }

            // update Branch Predictor
            self.branch_predictor
                .mispredict_recovery(self.branch_correct_direction, self.branch_destination);

            // clean self.branch_recover flag
            self.branch_recover = false;
        }

        // start new simulation cycle
        let stall_load_use_hazard = self.pipe_data_forwarding();
        self.pipe_stage_wb();
        self.pipe_stage_mem();
        self.pipe_stage_exe();
        self.pipe_stage_decode(stall_load_use_hazard);
        self.pipe_stage_fetch();

        // tick L1-I$ and L1-D$
        self.icache.tick();
        self.dcache.tick();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::super::mem::simple_mem::SimpleMem;
    use super::*;
    use crate::sim::elf;

    #[test]
    fn hello_world() {
        let elf::ProgramInfo {
            entry_pc,
            prog_body,
        } = elf::elf_loader(
            &"/home/ubuntu/workspace/SimplePipeSim/target/riscv32im-unknown-none-elf/debug/hello"
                .to_string(),
        );

        let mem = Rc::new(RefCell::new(SimpleMem::new(prog_body)));
        let mut cpu = PipelineProcessor::new(entry_pc, Rc::clone(&mem));

        while cpu.halt == false {
            cpu.tick();
            mem.borrow_mut().tick();
        }
    }

    // #[test]
    fn mem_to_exe_normal_hazard() {
        todo!();
    }

    // #[test]
    fn wb_to_exe_normal_hazard() {
        todo!();
    }

    // #[test]
    fn load_use_hazard() {
        todo!();
    }
}
