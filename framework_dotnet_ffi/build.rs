use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = Path::new("csharp");
    fs::create_dir_all(output_dir)?;
    println!("cargo:rerun-if-changed=src/lib.rs");

    csbindgen::Builder::default()
        .input_extern_file("src/lib.rs")
        .csharp_dll_name("framework_dotnet_ffi")
        .csharp_namespace("Framework.System.Interop")
        .csharp_class_name("NativeMethods")
        .csharp_class_accessibility("internal")
        .generate_csharp_file(output_dir.join("NativeMethods.g.cs"))?;

    Ok(())
}
