fn main() {
    #[cfg(feature = "windows")]
    static_vcruntime::metabuild();
}
