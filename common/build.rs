use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["../proto/fenris.proto"], &["../proto/"])?;

    println!("cargo:rerun-if-changed=../proto/fenris.proto");

    Ok(())
}
