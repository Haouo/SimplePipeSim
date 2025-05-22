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

/// A struct for the composition of PC and Branch Prediction Result
struct InstFetchInfo {
    pc: u32,
    bp_result: BranchPredictResult,
}

/// Public struct `PipelineProcessor`
///
/// This struct contains the necessary information to imitate a classic 5 stage RISC-V pipeline processor.
pub struct PipelineProcessor {
    // IF-Stage instruction fetch PC
    if_info: InstFetchInfo, // composition of PC with its Branch Predict Result
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

    // waiting information about memory related stages (ID and MEM)
    if_is_waiting: Option<MemoryReqType>, // second variable is the branch prediction result of the IF PC
    mem_is_waiting: Option<MemoryReqType>,

    // information for branch misprediction
    branch_recover: bool,
    branch_destination: u32,
    branch_flushes: usize,

    // imitate stall for integer mul/div instructions
    int_mul_div_stall_countdown: u8,

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
            if_info: InstFetchInfo {
                pc: init_pc,
                bp_result: BranchPredictResult::default(),
            },
            branch_predictor: Box::new(super::branch_predictor::dummy::Predictor),
            id_regs: [0; 32],
            id_op: None,
            exe_op: None,
            mem_op: None,
            wb_op: None,
            if_is_waiting: None,
            mem_is_waiting: None,
            branch_recover: false,
            branch_destination: 0,
            branch_flushes: 0,
            int_mul_div_stall_countdown: 0,
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
        // stall current stage, because downstream is stalled
        if self.id_op.is_some()
            | self.exe_op.is_some()
            | self.mem_op.is_some()
            | self.wb_op.is_some()
        {
            return;
        }

        // for caching load data from L1-I$
        let mut load_data = 0u32;

        // check whether there is inflight memory load request
        if let Some(ref mem_req_ref) = self.if_is_waiting {
            if mem_req_ref.get_done() {
                return;
            }

            // inflight request is done
            let load_data_arr: [u8; 4] = mem_req_ref.get_load_req_ref().buffer.borrow()[0..=3]
                .try_into()
                .expect("The length of load data from L1-I$ should be 4.");
            load_data = u32::from_le_bytes(load_data_arr);
        }

        // prepare for issuing new load request to L1-I$
        if self.mem_is_waiting.is_none() {
            let new_load_req = MemoryReqType::Load(MemoryLoadReq {
                addr: self.if_info.pc,
                len: 4,
                done: Rc::new(Cell::new(false)),
                buffer: Rc::new(RefCell::new(Box::from([0u8; 4]))),
            });
            assert!(
                self.icache.try_register_req(&new_load_req).is_ok(),
                "Memory request to L1-I$ should not fail, because L1-I$ is not shared resource."
            );

            // need to wait the inflight request
            if !new_load_req.get_done() {
                self.if_is_waiting = Some(new_load_req);
                return;
            }
            // cache is hit at current cycle
            let load_data_arr: [u8; 4] = new_load_req.get_load_req_ref().buffer.borrow()[0..=3]
                .try_into()
                .expect("The length of load data from L1-I$ should be 4.");
            load_data = u32::from_le_bytes(load_data_arr);
        }

        // make branch prediction for next cycle
        let next_cycle_bp_result = self.branch_predictor.branch_predict(self.if_info.pc);
        // tranfer raw binary data to Instructrion and make PreDecodeMicroOp
        self.id_op = Some(PreDecodeMicroOp {
            pc: self.if_info.pc,
            inst: Instruction::raw_binary_to_inst(load_data),
            bp_result: self.if_info.bp_result,
            ..Default::default()
        });
        // update PC according to branch prediction result
        if next_cycle_bp_result.direction {
            // taken
            self.if_info = InstFetchInfo {
                pc: next_cycle_bp_result.addr.unwrap(),
                bp_result: next_cycle_bp_result,
            };
        } else {
            // not-taken
            self.if_info = InstFetchInfo {
                pc: self.if_info.pc + 4,
                bp_result: BranchPredictResult::default(),
            };
        }
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
        // stall if `self.id_op` is not ready (means that ID stage is still waiting for I$)
        if self.id_op.is_none() {
            return;
        }
        // stall if there is Load-Use Hazard which needs additional stall even with data forwarding
        if additional_stall {
            return;
        }

        // perform pre-decoding logic
        let mut current_op = self.id_op.take().unwrap(); // note take() at here
        let inst_ref = &current_op.inst;

        // Pre-decode
        match inst_ref {
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
                current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                current_op.rs2 = Some((inst.rs2(), self.id_regs[inst.rs2() as usize]));
                current_op.rd_index = Some(inst.rd());
                current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                current_op.alu_op2_sel = AluOpTwoSelect::RegRs2;
                current_op.alu_result_as_rd_dst_value = true;
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
                current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
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
                current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                current_op.immediate_signext = inst.imm_sign_ext();
                current_op.rd_index = Some(inst.rd());
                current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                current_op.alu_op_type = AluOpTypes::Add;
                current_op.is_mem = true;
            }

            // STORE
            Instruction::Sb(inst) | Instruction::Sh(inst) | Instruction::Sw(inst) => {
                current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                current_op.rs2 = Some((inst.rs2(), self.id_regs[inst.rs2() as usize]));
                current_op.immediate_signext = inst.sign_ext();
                current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                current_op.alu_op_type = AluOpTypes::Add;
                current_op.is_mem = true;
                current_op.is_store = true;
            }

            // BRANCH
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
                current_op.alu_op1_sel = AluOpOneSelect::CurrentPc;
                current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                current_op.alu_op_type = AluOpTypes::Add;
                current_op.rd_write_value = Some(current_op.pc + 4);
                current_op.is_branch = true;
            }
            // JALR
            Instruction::Jalr(inst) => {
                current_op.rs1 = Some((inst.rs1(), self.id_regs[inst.rs1() as usize]));
                current_op.immediate_signext = inst.imm_sign_ext();
                current_op.alu_op1_sel = AluOpOneSelect::RegRs1;
                current_op.alu_op2_sel = AluOpTwoSelect::ImmSignExt;
                current_op.alu_op_type = AluOpTypes::Add;
                current_op.rd_write_value = Some(current_op.pc + 4);
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
            Instruction::Fence(inst) => {
                todo!();
            }

            // SYSTEM
            Instruction::Ecall(inst) => {
                current_op.is_env_call = true;
            }

            // We capture illegal instructions in the ID stage and make the program panic.
            Instruction::Illegal(raw_inst) => panic!("Unknown instruction: {:#08X}", raw_inst),
        }

        // stage 2 pre-decode (fine-grained) for OP and OP-IMM OPCODE types
        // it decides the ALU Operation types
        match inst_ref {
            // Addition
            Instruction::Add(_) | Instruction::Addi(_) => current_op.alu_op_type = AluOpTypes::Add,
            // Subtraction
            Instruction::Sub(_) => current_op.alu_op_type = AluOpTypes::Sub,
            // Shift Left Logically
            Instruction::Sll(_) | Instruction::Slli(_) => current_op.alu_op_type = AluOpTypes::Sll,
            // Set on Less-than
            Instruction::Slt(_) | Instruction::Slti(_) => current_op.alu_op_type = AluOpTypes::Slt,
            // Set on Less-than Unsigned
            Instruction::Sltu(_) | Instruction::Sltiu(_) => {
                current_op.alu_op_type = AluOpTypes::Sltu
            }
            // Bitwise XOR
            Instruction::Xor(_) | Instruction::Xori(_) => current_op.alu_op_type = AluOpTypes::Xor,
            // Shift Right Logically
            Instruction::Srl(_) | Instruction::Srli(_) => current_op.alu_op_type = AluOpTypes::Srl,
            // Shift Right Arithmetically
            Instruction::Sra(_) | Instruction::Srai(_) => current_op.alu_op_type = AluOpTypes::Sra,
            // Bitwise OR
            Instruction::Or(_) | Instruction::Ori(_) => current_op.alu_op_type = AluOpTypes::Or,
            // Bitwise AND
            Instruction::And(_) | Instruction::Andi(_) => current_op.alu_op_type = AluOpTypes::And,
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

        // pass uOp to next stage (EXE stage)
        self.exe_op = Some(current_op);

        // **Ps**
        // In hardware implementation, stage 1 and 2 can be implemented in parallel.
        // In other words, the hardware decoder can have two parallel decoding path because they are independent logically.
        // The first performs common decoding logics, and the second performs specific decoding logic for determining ALU OP-Types.
    }

    /// ### Instruction Execute Pipeline Stage Function
    fn pipe_stage_exe(&mut self) {
        // decrease Mul/Div/Rem countdown counter in need, and stall if the counter is not zero
        if self.int_mul_div_stall_countdown > 0 {
            self.int_mul_div_stall_countdown -= 1;
            return;
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
            // numerical calculations
            let op1: u32 = match current_op.alu_op1_sel {
                AluOpOneSelect::RegRs1 => current_op.rs1.unwrap().1,
                AluOpOneSelect::CurrentPc => current_op.pc,
                AluOpOneSelect::Zero => 0u32,
            };
            let op2: u32 = match current_op.alu_op2_sel {
                AluOpTwoSelect::RegRs2 => current_op.rs2.unwrap().1,
                AluOpTwoSelect::ImmSignExt => current_op.immediate_signext,
            };
            match current_op.alu_op_type {
                AluOpTypes::Add => current_op.alu_result = op1 + op2,
                AluOpTypes::Sub => current_op.alu_result = op1 - op2,
                AluOpTypes::Sll => {}
                AluOpTypes::Slt => {
                    current_op.alu_result = if (op1 as i32) < (op2 as i32) { 1 } else { 0 }
                }
                AluOpTypes::Sltu => current_op.alu_result = if op1 < op2 { 1 } else { 0 },
                AluOpTypes::Xor => current_op.alu_result = op1 ^ op2,
                AluOpTypes::Srl => current_op.alu_result = op1 << (op2 & 0x1f),
                AluOpTypes::Sra => current_op.alu_result = ((op1 as i32) << (op2 & 0x1f)) as u32,
                AluOpTypes::Or => current_op.alu_result = op1 | op2,
                AluOpTypes::And => current_op.alu_result = op1 & op2,
                AluOpTypes::Mul => {
                    current_op.alu_result = (((op1 as i32) as i64) * ((op2 as i32) as i64)) as u32
                }
                AluOpTypes::Mulh => {
                    current_op.alu_result =
                        ((((op1 as i32) as i64) * ((op2 as i32) as i64)) >> 32) as u32
                }
                AluOpTypes::Mulhu => {
                    current_op.alu_result = ((op1 as u64) * (op2 as u64) >> 32) as u32
                }
                AluOpTypes::Mulhsu => {
                    current_op.alu_result = (((op1 as i32) as i64) * (op2 as i64) >> 32) as u32
                }
                AluOpTypes::Div => {
                    current_op.alu_result = if op2 != 0 {
                        (((op1 as i32) as i64) / ((op1 as i32) as i64)) as u32
                    } else {
                        0xffff_ffffu32
                    }
                }
                AluOpTypes::Divu => {
                    current_op.alu_result = if op2 != 0 {
                        ((op1 as u64) / (op2 as u64)) as u32
                    } else {
                        0xffff_ffffu32
                    }
                }
                AluOpTypes::Rem => {
                    current_op.alu_result = if op2 != 0 {
                        ((op1 as i32) % (op2 as i32)) as u32
                    } else {
                        op1
                    }
                }
                AluOpTypes::Remu => current_op.alu_result = if op2 != 0 { op1 % op2 } else { op1 },
            }
            if current_op.alu_result_as_rd_dst_value {
                current_op.rd_write_value = Some(current_op.alu_result);
            }

            // resolve control-flow instructions (conditional/unconditional branches)
            //
            // It have to judge whether the last branch prediction result is correct by comparing
            // current PC and the actual branch result.
            if current_op.is_branch {
                let mut taken = false;
                match current_op.inst {
                    // unconditional branch
                    Instruction::Jal(_) | Instruction::Jalr(_) => taken = true,
                    // conditional branch
                    Instruction::Beq(_) => taken = if op1 == op2 { true } else { false },
                    Instruction::Bne(_) => taken = if op1 != op2 { true } else { false },
                    Instruction::Blt(_) => {
                        taken = if (op1 as i32) < (op2 as i32) {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bge(_) => {
                        taken = if (op1 as i32) < (op2 as i32) {
                            true
                        } else {
                            false
                        }
                    }
                    Instruction::Bltu(_) => taken = if op1 < op2 { true } else { false },
                    Instruction::Bgeu(_) => taken = if op1 >= op2 { true } else { false },

                    // non-branch inst.
                    _ => {
                        unreachable!();
                    }
                }

                // check whether the last branch prediction is incorrect
                if current_op.bp_result.direction != taken {
                    self.branch_recover = true;
                    self.branch_destination = current_op.alu_result;
                    self.branch_flushes = 3;
                }
            }
        }
        // clean exe_op
        self.exe_op = None;
    }

    /// ### Memory Access Pipeline Stage Function
    fn pipe_stage_mem(&mut self) {
        // stall if there is no any job to do
        if self.mem_op.is_none() {
            return;
        }
        // stall if downstream stage is stalled
        if self.wb_op.is_some() {
            return;
        }

        // temporal storage for load data
        let mut load_data: Option<u32> = None;

        // check inflight memory request
        if let Some(ref inflight_mem_req) = self.mem_is_waiting {
            // not done
            if !inflight_mem_req.get_done() {
                return;
            }
            // inflight request is done and it is LOAD inst.
            if let MemoryReqType::Load(load_req) = inflight_mem_req {
                let load_data_arr: [u8; 4] = load_req.buffer.borrow()[0..=3]
                    .try_into()
                    .expect("The length of load data from L1-D$ should be 4");
                load_data = Some(u32::from_le_bytes(load_data_arr));
            }
        }

        // issue new request to L1-D$
        if let Some(ref current_op) = self.mem_op {
            if self.mem_op.as_ref().unwrap().is_mem && self.mem_is_waiting.is_none() {
                let mem_addr = current_op.alu_result;
                let new_mem_req = if current_op.is_store {
                    let store_data: [u8; 4] = current_op.rs2.unwrap().1.to_le_bytes();
                    MemoryReqType::Store(MemoryStoreReq {
                        addr: mem_addr,
                        len: 4,
                        store_data: Box::from(store_data),
                        done: Rc::new(Cell::new(false)),
                    })
                } else {
                    MemoryReqType::Load(MemoryLoadReq {
                        addr: mem_addr,
                        len: 4,
                        done: Rc::new(Cell::new(false)),
                        buffer: Rc::new(RefCell::new(Box::from([0u8; 4]))),
                    })
                };
                // register new memory request
                assert!(self.dcache.try_register_req(&new_mem_req).is_ok(), "Memory request to L1-D$ should not fail, bacause L1-D$ is not shared resource.");

                // check whether cache is hit in the current cycle
                if !new_mem_req.get_done() {
                    // cache miss
                    self.mem_is_waiting = Some(new_mem_req);
                    return;
                }

                // hit in current cycle
                if !current_op.is_store {
                    // LOAD inst.
                    let load_data_arr: [u8; 4] = new_mem_req.get_load_req_ref().buffer.borrow()
                        [0..=3]
                        .try_into()
                        .expect("The length of load data from L1-D$ should be 4.");
                    load_data = Some(u32::from_le_bytes(load_data_arr));
                }
                // it does not to do anything for STORE isnt.
            }
        }

        // get load data from L1-D$
        self.mem_op.as_mut().unwrap().rd_write_value =
            Some(load_data.expect("Load data should not be None."));

        // clean mem_op
        self.mem_op = None;
    }

    /// ### Architectural Register File Write-back Pipeline Stage Function
    fn pipe_stage_wb(&mut self) {
        // stall if there is o any job to do
        if self.wb_op.is_none() {
            return;
        }

        // handle current instruction in WB stage
        let current_op = self.wb_op.take().unwrap();
        if current_op.rd_index.is_none() {
            return;
        }
        self.id_regs[current_op.rd_index.unwrap() as usize] = current_op.rd_write_value.expect(
            "The write value into $rd register should not be None when rd_index is not None. It makes no sense.",
        );
    }

    /// Try to solve RAW hazards before the start of a new cycle by using data forwarding
    fn pipe_data_forwarding(&mut self) -> bool {
        let exe_op_mut_ref = self.exe_op.as_mut().unwrap();
        // Check scenario 1: WB -> EXE Forwarding Path
        if self
            .wb_op
            .as_ref()
            .is_some_and(|x| x.rd_index.is_some_and(|x| x != 0))
        {
            let wb_rd_index = self.wb_op.as_ref().unwrap().rd_index.unwrap();
            if exe_op_mut_ref.rs1.is_some_and(|x| x.0 == wb_rd_index) {
                exe_op_mut_ref.rs1 = Some((
                    exe_op_mut_ref.rs1.unwrap().0,
                    self.wb_op.as_ref().unwrap().rd_write_value.unwrap(),
                ));
            }
            if exe_op_mut_ref.rs2.is_some_and(|x| x.0 == wb_rd_index) {
                exe_op_mut_ref.rs2 = Some((
                    exe_op_mut_ref.rs2.unwrap().0,
                    self.wb_op.as_ref().unwrap().rd_write_value.unwrap(),
                ));
            }
        }

        // Check scenario 2: MEM -> EXE Forwarding Path
        // it might override the forwarding data of scenario 1
        // because the latter instruction (in MEM stage) has newest data
        if self
            .mem_op
            .as_ref()
            .is_some_and(|x| x.rd_index.is_some_and(|x| x != 0))
        {
            let mem_rd_index = self.mem_op.as_ref().unwrap().rd_index.unwrap();
            if exe_op_mut_ref.rs1.unwrap().0 == mem_rd_index {
                //
            }
            if exe_op_mut_ref.rs2.unwrap().0 == mem_rd_index {
                //
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
        let stall_load_use_hazard = self.pipe_data_forwarding();
        self.pipe_stage_wb();
        self.pipe_stage_mem();
        self.pipe_stage_exe();
        self.pipe_stage_decode(stall_load_use_hazard);
        self.pipe_stage_fetch();

        // handle branch recovery for branch miss-prediction
        if self.branch_recover {
            // @TODO
        }
    }
}
