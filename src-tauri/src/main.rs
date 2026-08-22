#[cfg(feature = "gui")]
fn main() {
    ohmyworkpanel_lib::run();
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("This binary requires the 'gui' feature. Use 'ohmyworkpanel-server' for headless mode.");
    std::process::exit(1);
}
