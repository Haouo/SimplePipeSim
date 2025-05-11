use std::error::Error;
use std::fs;

use object;

pub fn elf_loader(file: &String) -> Result<Vec<u8>, Box<dyn Error>> {
    let object_fs_read = fs::read(file)?;
    let object_parse = object::File::parse(&*object_fs_read)?;
    todo!();
}

mod unit_tests {
    use super::*;

    #[test]
    fn read_example_object_file() {
        todo!();
    }
}
