#[cfg(feature = "gui")]
fn main() {
    linlis_work_panel_lib::run();
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("This binary requires the 'gui' feature. Use 'linlis-work-panel-server' for headless mode.");
    std::process::exit(1);
}
