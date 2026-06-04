fn main() {
    if let Err(error) = mcpstore_cli::tui::run() {
        eprintln!("[错误] {error}");
        std::process::exit(1);
    }
}
