use super::branch_predictor::BranchPredictResult;
use crate::riscv::format_types::IType;
use crate::riscv::instruction::Instruction;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AluOpTypes {
    #[default]
    Add,
    Sll,
    Slt,
    Sltu,
    Xor,
    Srl,
    Or,
    And,
    Sub,
    Sra,
    Mul,
    Mulh,
    Mulhu,
    Mulhsu,
    Div,
    Divu,
    Rem,
    Remu,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AluOpOneSelect {
    #[default]
    RegRs1,
    CurrentPc,
    Zero,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AluOpTwoSelect {
    #[default]
    RegRs2,
    ImmSignExt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryAccessWidth {
    Byte,
    HalfWord,
    Word,
}

impl MemoryAccessWidth {
    pub fn len(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::HalfWord => 2,
            Self::Word => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadExtension {
    Sign,
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryAccess {
    Load {
        addr: u32,
        width: MemoryAccessWidth,
        extension: LoadExtension,
    },
    Store {
        addr: u32,
        width: MemoryAccessWidth,
        value: u32,
    },
}

impl MemoryAccess {
    pub fn addr(self) -> u32 {
        match self {
            Self::Load { addr, .. } | Self::Store { addr, .. } => addr,
        }
    }

    pub fn len(self) -> usize {
        match self {
            Self::Load { width, .. } | Self::Store { width, .. } => width.len(),
        }
    }

    pub fn store_value(self) -> Option<u32> {
        match self {
            Self::Store { value, .. } => Some(value),
            Self::Load { .. } => None,
        }
    }

    pub fn is_load(self) -> bool {
        matches!(self, Self::Load { .. })
    }

    pub fn loaded_value(self, bytes: &[u8]) -> u32 {
        let Self::Load {
            width, extension, ..
        } = self
        else {
            panic!("Only load accesses can materialize loaded bytes");
        };

        assert_eq!(
            bytes.len(),
            width.len(),
            "A {:?} load should complete with {} bytes",
            width,
            width.len()
        );

        let mut word = [0u8; 4];
        word[..bytes.len()].copy_from_slice(bytes);
        let value = u32::from_le_bytes(word);

        match (width, extension) {
            (MemoryAccessWidth::Byte, LoadExtension::Sign) => ((value as u8) as i8) as u32,
            (MemoryAccessWidth::HalfWord, LoadExtension::Sign) => ((value as u16) as i16) as u32,
            (_, LoadExtension::Zero) | (MemoryAccessWidth::Word, LoadExtension::Sign) => value,
        }
    }
}

#[derive(Clone, Copy)]
pub struct BranchResolutionInputs {
    pub inst: Instruction,
    pub pc: u32,
    pub predicted: BranchPredictResult,
    pub rs1: u32,
    pub rs2: u32,
    pub target: u32,
}

impl BranchResolutionInputs {
    pub fn needs_recovery(self, actual_direction: bool) -> bool {
        self.predicted.direction != actual_direction
            || (actual_direction && self.predicted.addr != self.target)
    }

    pub fn recovery_destination(self, actual_direction: bool) -> u32 {
        if actual_direction {
            self.target
        } else {
            self.pc + 4
        }
    }
}

#[derive(Default)]
pub struct PreDecodeMicroOp {
    // basic info
    pub raw_inst: u32,     // generated in IF
    pub inst: Instruction, // generated in IF
    pub pc: u32,           // generated in IF

    // register sources information
    pub rs1: Option<(u8, u32)>, // might be generated in ID
    pub rs2: Option<(u8, u32)>, // might be generated in ID
    pub immediate_signext: u32, // might be generated in ID

    // EXE stage output
    pub alu_op_type: AluOpTypes,          // generated in ID
    pub alu_op1_sel: AluOpOneSelect,      // generated in ID
    pub alu_op2_sel: AluOpTwoSelect,      // generated in ID
    pub alu_result: u32, // generated in EXE (the pipeline might take many cycles to calculate)
    pub alu_result_as_rd_dst_value: bool, // whether the calculation result of ALU is write-value of rd register

    // register destination information
    // The index of rd is generated in ID,
    pub rd_index: Option<u8>,
    pub rd_write_value: Option<u32>, // might be generated in ID, EXE or MEM

    // branch prediction information
    pub bp_result: Option<BranchPredictResult>, // generated in IF

    // special flag: placeholder
    // The current uOp should be viewed as a normal instruction (NOP), which means that
    // it can take place of some pipeline stage and preventing from propagating of the instruction at the previous stage,
    // while the uOp with placeholder flags being true should not be counted into retired instructions.
    pub placeholder: bool,
}

impl PreDecodeMicroOp {
    pub fn fetched(raw_inst: u32, inst: Instruction, pc: u32) -> Self {
        Self {
            raw_inst,
            inst,
            pc,
            ..Default::default()
        }
    }

    pub fn fetched_branch(
        raw_inst: u32,
        inst: Instruction,
        pc: u32,
        bp_result: BranchPredictResult,
    ) -> Self {
        Self {
            bp_result: Some(bp_result),
            ..Self::fetched(raw_inst, inst, pc)
        }
    }

    pub fn generate_nop() -> Self {
        Self {
            pc: 0xffffffffu32,
            inst: Instruction::Addi(IType(0x00000013u32)),
            rs1: Some((0, 0)),
            rs2: Some((0, 0)),
            ..Default::default()
        }
    }

    pub fn placeholder_nop() -> Self {
        Self {
            placeholder: true,
            ..Self::generate_nop()
        }
    }

    pub fn memory_access(&self) -> Option<MemoryAccess> {
        match self.inst {
            Instruction::Lb(_) => Some(MemoryAccess::Load {
                addr: self.alu_result,
                width: MemoryAccessWidth::Byte,
                extension: LoadExtension::Sign,
            }),
            Instruction::Lh(_) => Some(MemoryAccess::Load {
                addr: self.alu_result,
                width: MemoryAccessWidth::HalfWord,
                extension: LoadExtension::Sign,
            }),
            Instruction::Lw(_) => Some(MemoryAccess::Load {
                addr: self.alu_result,
                width: MemoryAccessWidth::Word,
                extension: LoadExtension::Zero,
            }),
            Instruction::Lbu(_) => Some(MemoryAccess::Load {
                addr: self.alu_result,
                width: MemoryAccessWidth::Byte,
                extension: LoadExtension::Zero,
            }),
            Instruction::Lhu(_) => Some(MemoryAccess::Load {
                addr: self.alu_result,
                width: MemoryAccessWidth::HalfWord,
                extension: LoadExtension::Zero,
            }),
            Instruction::Sb(_) => Some(MemoryAccess::Store {
                addr: self.alu_result,
                width: MemoryAccessWidth::Byte,
                value: self
                    .rs2
                    .expect("store should carry rs2 value after decode")
                    .1,
            }),
            Instruction::Sh(_) => Some(MemoryAccess::Store {
                addr: self.alu_result,
                width: MemoryAccessWidth::HalfWord,
                value: self
                    .rs2
                    .expect("store should carry rs2 value after decode")
                    .1,
            }),
            Instruction::Sw(_) => Some(MemoryAccess::Store {
                addr: self.alu_result,
                width: MemoryAccessWidth::Word,
                value: self
                    .rs2
                    .expect("store should carry rs2 value after decode")
                    .1,
            }),
            _ => None,
        }
    }

    pub fn is_load(&self) -> bool {
        self.memory_access().is_some_and(MemoryAccess::is_load)
    }

    pub fn branch_resolution_inputs(&self) -> Option<BranchResolutionInputs> {
        match self.inst {
            Instruction::Beq(_)
            | Instruction::Bne(_)
            | Instruction::Blt(_)
            | Instruction::Bge(_)
            | Instruction::Bltu(_)
            | Instruction::Bgeu(_)
            | Instruction::Jal(_)
            | Instruction::Jalr(_) => Some(BranchResolutionInputs {
                inst: self.inst,
                pc: self.pc,
                predicted: self
                    .bp_result
                    .expect("control-flow instructions should carry a branch prediction"),
                rs1: self.rs1.unwrap_or((0, 0)).1,
                rs2: self.rs2.unwrap_or((0, 0)).1,
                target: self.alu_result,
            }),
            _ => None,
        }
    }

    pub fn set_writeback_value(&mut self, value: u32) {
        self.rd_write_value = Some(value);
    }

    pub fn writeback(&self) -> Option<(u8, u32)> {
        self.rd_index.zip(self.rd_write_value)
    }

    pub fn destination_register(&self) -> Option<u8> {
        self.rd_index
    }

    pub fn is_placeholder(&self) -> bool {
        self.placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_materialization_sign_and_zero_extends_bytes() {
        let signed = MemoryAccess::Load {
            addr: 0,
            width: MemoryAccessWidth::Byte,
            extension: LoadExtension::Sign,
        };
        let zero = MemoryAccess::Load {
            addr: 0,
            width: MemoryAccessWidth::Byte,
            extension: LoadExtension::Zero,
        };

        assert_eq!(signed.loaded_value(&[0x80]), 0xffff_ff80);
        assert_eq!(zero.loaded_value(&[0x80]), 0x80);
    }
}
