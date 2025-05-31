use object::{Object, ObjectSegment};

use std::path::Path;

const MEM_SIZE: usize = 0x40000;

pub struct ProgramInfo {
    pub entry_pc: u32,
    pub prog_body: Vec<u8>,
}

pub fn elf_loader(file_path: &Path) -> ProgramInfo {
    let file = std::fs::read(file_path).expect("Cannot read file!");
    let object_parse =
        object::File::parse(file.as_slice()).expect("Cannot parse the given ELF file!");

    // Check ELF format
    match object_parse {
        object::File::Elf32(_) => {}
        _ => panic!("The given ELF file is not EFL32 format!"),
    }
    // check ELF architecture
    match object_parse.architecture() {
        object::Architecture::Riscv32 => {}
        _ => panic!("Invalid architecture for the ELF file! (only RISCV32 is allowed)"),
    }
    // check the kind of the ELF
    match object_parse.kind() {
        object::ObjectKind::Executable => {}
        _ => panic!("Invalid kind of the ELF file! (only executable is allowed)"),
    }
    // check endianness (only little is allowed)
    assert!(
        object_parse.is_little_endian(),
        "The given ELF file is not little-endian!"
    );

    // load sections into Vec<u8>
    let mut prog: Vec<u8> = vec![0u8; MEM_SIZE]; // 128-KiB Memory
    for seg in object_parse.segments() {
        assert!(seg.address() + seg.size() < MEM_SIZE as u64); // memory boundary check
        let start_addr = seg.address() as usize;
        let end_addr = (seg.address() + seg.size()) as usize;
        let seg_data_len = seg.data().unwrap().len();
        let len_diff = (end_addr - start_addr) - seg_data_len;

        if len_diff > 0 {
            if seg.data().unwrap().len() == 0 {
                // zero padding
                prog[start_addr..end_addr].copy_from_slice(&*vec![0u8; seg.size() as usize]);
            } else {
                prog[start_addr..(start_addr + seg_data_len)].copy_from_slice(seg.data().unwrap());
                prog[(start_addr + seg_data_len)..end_addr].copy_from_slice(&*vec![0u8; len_diff]);
                // zero padding
            }
        } else {
            prog[start_addr..end_addr].copy_from_slice(seg.data().unwrap());
        }
    }

    // return value
    ProgramInfo {
        entry_pc: object_parse.entry() as u32,
        prog_body: prog,
    }
}
