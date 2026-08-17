fn main() {
    if let Some(exit_code) = vibe_inspector_lib::cli::run_if_requested() {
        std::process::exit(exit_code);
    }
    vibe_inspector_lib::run();
}
