fn main() {
    // if compiling for windows bundle the rc file
    if cfg!(windows) {
        windres::Build::new().compile("res/res.rc").unwrap();
        windows::build!(windows::win32::shell::SetCurrentProcessExplicitAppUserModelID);
    }
}
