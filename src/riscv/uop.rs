/// It defines the micro-Op for the supported instructions in our pipeline simulator.
///
/// The uOp is generated at the decode stage within the pipeline.
pub struct UOp {
    pc: u32,
    inst: u32,
    opcode: u8,
    func3: u8,
    func7: u8,
    func12: u16,
    is_mem: bool,
    is_branch: bool,
    rs1_index: u8,
    rs2_index: u8,
    rd_index: u8,
}
