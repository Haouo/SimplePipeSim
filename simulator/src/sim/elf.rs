use object::{Object, ObjectSegment};

const MEM_SIZE: usize = 0x40000;

pub fn elf_loader(file_path: &String) -> Vec<u8> {
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
    let mut ret: Vec<u8> = vec![0u8; MEM_SIZE]; // 128-KiB Memory
    for seg in object_parse.segments() {
        assert!(seg.address() + seg.size() < MEM_SIZE as u64); // memory boundary check
        let start_addr = seg.address() as usize;
        let end_addr = (seg.address() + seg.size()) as usize;
        ret[start_addr..end_addr].copy_from_slice(
            seg.data()
                .expect("Data of loadable segment should not be None!"),
        );
    }
    ret
}
