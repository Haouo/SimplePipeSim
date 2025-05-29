use std::{
    env, fs,
    io::{self, Write},
    path,
};

fn main() -> Result<(), io::Error> {
    let out_dir = path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search={}", out_dir.display());
    fs::File::create(out_dir.join("link.ld"))?.write_all(include_bytes!("bin/link.ld"))?;
    Ok(())
}
