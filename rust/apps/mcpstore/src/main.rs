fn main() {
    if let Err(error) = mcpstore_cli::cli_app::run() {
        eprintln!("[错误] {error}");
        std::process::exit(1);
    }
}
