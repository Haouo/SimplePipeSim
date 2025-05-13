pub enum OpcodeMap {
    Op,
    OpImm,
    Load,
    Store,
    Branch,
    Jal,
    Jalr,
    Lui,
    Auipc,
    MiscMem,
    System,
}

impl TryFrom<u8> for OpcodeMap {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b0110011 => Ok(Self::Op),
            0b0010011 => Ok(Self::OpImm),
            0b0000011 => Ok(Self::Load),
            0b0100011 => Ok(Self::Store),
            0b1100011 => Ok(Self::Branch),
            0b1101111 => Ok(Self::Jal),
            0b1100111 => Ok(Self::Jalr),
            0b0110111 => Ok(Self::Lui),
            0b0010111 => Ok(Self::Auipc),
            0b0001111 => Ok(Self::MiscMem),
            0b1110011 => Ok(Self::System),
            _ => Err(()),
        }
    }
}
