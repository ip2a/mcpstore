fn main() {
    if let Err(error) = mcpstore_cli::tui::run() {
        eprintln!("[Error] {error}");
        std::process::exit(1);
    }
}
