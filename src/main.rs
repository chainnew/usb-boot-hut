use clap::Parser;
use usb_boot_hut::cli::{Cli, commands};
use colored::*;
use std::process;

fn main() {
    // Parse CLI arguments
    let cli = Cli::parse();
    
    // Initialize logger
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .init();
    
    // Check if running as root (required for most operations)
    #[cfg(target_os = "linux")]
    if !is_root() && needs_root(&cli) {
        eprintln!("{}", "Error: This operation requires root privileges.".red());
        eprintln!("Please run with sudo: {}", format!("sudo {}", std::env::args().collect::<Vec<_>>().join(" ")).cyan());
        process::exit(1);
    }
    
    // Run the command
    if let Err(e) = commands::run(cli) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}

#[cfg(target_os = "linux")]
fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(target_os = "linux"))]
fn is_root() -> bool {
    // On non-Linux platforms, we'll need platform-specific checks
    false
}

#[allow(dead_code)]
fn needs_root(cli: &Cli) -> bool {
    use usb_boot_hut::cli::Commands;
    
    match &cli.command {
        Commands::Format { .. } |
        Commands::Unlock { .. } |
        Commands::Lock { .. } |
        Commands::Add { .. } |
        Commands::Remove { .. } |
        Commands::Clean { .. } |
        Commands::UpdateGrub { .. } => true,
        
        Commands::List { .. } |
        Commands::Devices { .. } |
        Commands::Config { .. } |
        Commands::Status { .. } |
        Commands::Verify { .. } => false,
    }
}
