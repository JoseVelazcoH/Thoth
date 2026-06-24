fn main() {
    match thoth::cli::run() {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("tth: {e}");
            std::process::exit(1);
        }
    }
}
