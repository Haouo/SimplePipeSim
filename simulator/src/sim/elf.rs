use std::error::Error;
use std::fs;

use object::{Object, ObjectSection, ObjectSegment};

pub fn elf_loader(file_path: &String) -> Result<Vec<u8>, Box<dyn Error>> {
    let file = fs::read(file_path)?;
    let object_parse = object::File::parse(file.as_slice())?;

    // Check ELF format
    match object_parse {
        object::File::Elf32(_) => {}
        _ => {
            panic!("Invalid ELF format!");
        }
    }
    // check ELF architecture
    match object_parse.architecture() {
        object::Architecture::Riscv32 => {}
        _ => {
            panic!("Invalid architecture for the ELF file!");
        }
    }
    // check the kind of the ELF
    match object_parse.kind() {
        object::ObjectKind::Executable => {}
        _ => {
            panic!("Invalid kind of the ELF file!");
        }
    }
    // check endianness (only little is allowed)
    assert!(object_parse.is_little_endian());

    // load sections into Vec<u8>
    let ret: Vec<u8> = Vec::with_capacity(0x40000); // 128-KiB Memory
    for seg in object_parse.segments() {
        assert!(seg.address() + seg.size() < 0x40000); // memory boundary check
        let seg_data = seg.data().unwrap();
        todo!();
    }
    Ok(ret)
}
