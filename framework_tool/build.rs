use {
    std::{env, io},
    winresource::WindowsResource,
};

fn main() -> io::Result<()> {
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            .set_icon("..\\res\\framework_startmenuicon.ico")
            .compile()?;
    }

    static_vcruntime::metabuild();

    Ok(())
}
