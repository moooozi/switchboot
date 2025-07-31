mod logic;

#[cfg(windows)]
mod windows;

fn main() {
    let mut args = std::env::args();
    let _exe = args.next();
    let rest: Vec<String> = args.collect();

    // Handle --daemon on all platforms
    if rest.len() == 1 && rest[0] == "--daemon" {
        logic::run_daemon();
        return;
    }

    #[cfg(windows)]
    {
        if rest.len() == 1 && rest[0].starts_with('/') {
            match rest[0].as_str() {
                "/service" => {
                    windows::launch_windows_service();
                    return;
                }
                "/installservice" => {
                    windows::install_service();
                    return;
                }
                "/uninstallservice" => {
                    windows::uninstall_service();
                    return;
                }
                _ => {}
            }
        }
    }

    std::process::exit(logic::run(rest));
}
