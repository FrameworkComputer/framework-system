fn main() {
    // if compiling for windows bundle the rc file
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windres::Build::new().compile("res/res.rc").unwrap();
        windows::build!(windows::win32::shell::SetCurrentProcessExplicitAppUserModelID);
    }
}
