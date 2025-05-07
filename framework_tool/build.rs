fn main() {
    // Add app icon
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        winresource::WindowsResource::new()
            .set_icon("..\\res\\framework_startmenuicon.ico")
            .compile()
            .unwrap();
    }

    if !cfg!(debug_assertions) {
        // Statically link vcruntime to allow running on clean install
        static_vcruntime::metabuild();

        // Embed resources file to force running as admin
        embed_resource::compile("framework_tool-manifest.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
