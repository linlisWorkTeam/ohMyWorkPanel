// 桌面发布版隐藏控制台窗口（release 下 windows 子系统；debug 保留方便看日志）
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

#[cfg(feature = "gui")]
fn main() {
    ohmyworkpanel_lib::run();
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("This binary requires the 'gui' feature. Use 'ohmyworkpanel-server' for headless mode.");
    std::process::exit(1);
}
