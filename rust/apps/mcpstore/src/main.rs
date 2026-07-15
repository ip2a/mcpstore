fn main() {
    if let Err(error) = mcpstore_cli::cli_app::run() {
        if error.is::<mcpstore_cli::commands::auth::JsonAuthError>() {
            eprintln!("{error}");
        } else {
            eprintln!("[错误] {error}");
        }
        std::process::exit(1);
    }
}
