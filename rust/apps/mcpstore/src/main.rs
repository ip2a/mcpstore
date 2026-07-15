fn main() {
    if let Err(error) = mcpstore_cli::cli_app::run() {
        if let Some(error) = error.downcast_ref::<mcpstore_cli::commands::task::TaskCommandError>()
        {
            eprintln!("{error}");
            std::process::exit(error.exit_code());
        }
        if error.is::<mcpstore_cli::commands::auth::JsonAuthError>() {
            eprintln!("{error}");
        } else {
            eprintln!("[错误] {error}");
        }
        std::process::exit(1);
    }
}
