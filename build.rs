fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app.ico");

        #[cfg(unix)]
        if std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc" {
            let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
            let rc_dir = out_dir.join("rc_toolkit");
            let _ = std::fs::create_dir_all(&rc_dir);
            let rc_exe = rc_dir.join("rc.exe");
            if !rc_exe.exists() {
                if let Some(path) = std::env::var_os("PATH").and_then(|p| {
                    std::env::split_paths(&p).find_map(|dir| {
                        let candidate = dir.join("llvm-rc");
                        if candidate.is_file() {
                            Some(candidate)
                        } else {
                            None
                        }
                    })
                }) {
                    let _ = std::os::unix::fs::symlink(path, &rc_exe);
                }
            }
            if rc_exe.exists() {
                res.set_toolkit_path(rc_dir.to_str().unwrap());
            }
        }

        res.compile().expect("failed to compile Windows resource");
    }
}
