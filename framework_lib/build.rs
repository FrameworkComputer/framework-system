fn main() {
    if cfg!(windows) {
        println!("cargo:rustc-link-lib=ws2_32");
    }
    built::write_built_file().expect("Failed to acquire build-time information");
}
