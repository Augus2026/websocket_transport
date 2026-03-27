use std::env;
use std::process;

fn print_usage() {
    println!("P2P SDK - P2P Communication Server/Client");
    println!();
    println!("Usage: p2p_sdk <mode>");
    println!();
    println!("Modes:");
    println!("  server    - Start P2P server");
    println!("  client    - Start P2P client");
    println!();
    println!("Options:");
    println!("  --help    - Show this help message");
    println!();
    println!("Examples:");
    println!("  p2p_sdk server");
    println!("  p2p_sdk client");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Check for help flag
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        process::exit(0);
    }

    if args.len() != 2 {
        print_usage();
        process::exit(1);
    }

    let mode = &args[1];

    match mode.as_str() {
        "server" => {
            if let Err(e) = p2p_sdk::server::run_server().await {
                eprintln!("Server error: {}", e);
                process::exit(1);
            }
        }
        "client" => {
            if let Err(e) = p2p_sdk::client::run_client().await {
                eprintln!("Client error: {}", e);
                process::exit(1);
            }
        }
        _ => {
            eprintln!("Unknown mode: {}", mode);
            println!();
            print_usage();
            process::exit(1);
        }
    }
}