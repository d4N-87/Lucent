mod cli;
mod gui;

fn main() {
    if std::env::args().any(|a| a == "--read-config") {
        cli::read_config();
        return;
    }
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--read-keys") {
        cli::read_keys(args.get(i + 1).map_or(".", String::as_str));
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--test-13") {
        cli::test_byte_13(&args[i + 1..]);
        return;
    }
    if args.iter().any(|a| a == "--effects") {
        cli::list_effects();
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--effect") {
        cli::set_effect(&args[i + 1..]);
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--write-mode") {
        cli::write_mode(&args[i + 1..]);
        return;
    }
    if std::env::args().any(|a| a == "--cli") {
        cli::run();
        return;
    }
    if let Err(e) = gui::run() {
        eprintln!("Errore avvio GUI: {e}");
    }
}
