fn main() {
    if !cfg!(debug_assertions) {
        // Statically link vcruntime to allow running on clean install
        static_vcruntime::metabuild();

        // Embed resources file to force running as admin
        embed_resource::compile("framework_tool-manifest.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
