pub mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "usb-boot-hut")]
#[command(author, version, about = "🔒 Secure USB Bootable Drive Manager", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    #[arg(short, long, global = true, help = "Enable verbose output")]
    pub verbose: bool,
    
    #[arg(long, global = true, help = "Disable colored output")]
    pub no_color: bool,
    
    #[arg(short, long, global = true, help = "Configuration file path")]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Format a USB drive and set it up for booting
    Format {
        /// Device path (e.g., /dev/sdb)
        device: PathBuf,
        
        /// Enable LUKS encryption for data partition
        #[arg(short, long)]
        encrypt: bool,
        
        /// Perform secure wipe before formatting
        #[arg(long)]
        secure_wipe: bool,
        
        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },
    
    /// Unlock an encrypted USB drive
    Unlock {
        /// Device path
        device: PathBuf,
        
        /// Mount point (optional, will auto-mount if not specified)
        #[arg(short, long)]
        mount: Option<PathBuf>,
    },
    
    /// Lock an encrypted USB drive
    Lock {
        /// Device path or mount point
        device: PathBuf,
    },
    
    /// Add an ISO to the USB drive
    Add {
        /// Path to the ISO file
        iso_file: PathBuf,
        
        /// Verify checksum (provide expected SHA256)
        #[arg(long)]
        verify: Option<String>,
        
        /// Category for the ISO
        #[arg(short, long)]
        category: Option<String>,
        
        /// Tags for the ISO (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },
    
    /// Remove an ISO from the USB drive
    Remove {
        /// ISO name or ID
        iso_name: String,
        
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    
    /// List ISOs on the USB drive
    List {
        /// Device path or mount point
        device: Option<PathBuf>,
        
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,
        
        /// Output format
        #[arg(short, long, default_value = "table")]
        format: ListFormat,
    },
    
    /// Verify ISO integrity
    Verify {
        /// Device path or mount point
        device: PathBuf,
        
        /// Specific ISO to verify (or "all")
        iso_name: Option<String>,
    },
    
    /// Clean junk files from the USB drive
    Clean {
        /// Device path or mount point
        device: PathBuf,
        
        /// Custom cleanup config file
        #[arg(long)]
        config: Option<PathBuf>,
        
        /// Perform dry run (show what would be deleted)
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    
    /// List available USB devices
    Devices {
        /// Show all devices (not just removable)
        #[arg(short, long)]
        all: bool,
        
        /// Output format
        #[arg(short, long, default_value = "table")]
        format: ListFormat,
    },
    
    /// Show USB drive status
    Status {
        /// Device path
        device: PathBuf,
    },
    
    /// Update GRUB configuration
    UpdateGrub {
        /// Device path or mount point
        device: PathBuf,
        
        /// Regenerate all entries
        #[arg(long)]
        regenerate: bool,
    },
    
    /// Securely wipe a USB drive (DANGEROUS!)
    Nuke {
        /// Device path to nuke
        device: PathBuf,
        
        /// Number of passes (default: 1)
        #[arg(short, long, default_value = "1")]
        passes: u8,
        
        /// Wipe pattern: random, zeros, dod (DoD 5220.22-M)
        #[arg(short = 'p', long, default_value = "random")]
        pattern: WipePattern,
        
        /// Skip ALL safety checks and confirmations (VERY DANGEROUS!)
        #[arg(long)]
        force: bool,
        
        /// Verify wipe after completion
        #[arg(long)]
        verify: bool,
    },
    
    /// Burn a Raspberry Pi or other disk image to SD card/USB
    Burn {
        /// Image file to burn (.img, .img.gz, .img.xz)
        image: PathBuf,
        
        /// Target device
        device: PathBuf,
        
        /// Skip verification after burning
        #[arg(long)]
        no_verify: bool,
        
        /// Enable SSH by creating ssh file in boot partition
        #[arg(long)]
        enable_ssh: bool,
        
        /// Set WiFi credentials (format: "SSID:password")
        #[arg(long)]
        wifi: Option<String>,
        
        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
        
        /// Eject device after burning
        #[arg(long)]
        eject: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    
    /// Edit configuration
    Edit {
        /// Key to edit (e.g., "default_timeout")
        key: String,
        
        /// New value
        value: String,
    },
    
    /// Reset to defaults
    Reset {
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    
    /// Generate default cleanup configuration
    GenerateCleanup {
        /// Output file path
        output: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ListFormat {
    Table,
    Json,
    Csv,
    Simple,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum WipePattern {
    /// Random data (most secure)
    Random,
    /// All zeros (fast)
    Zeros,
    /// DoD 5220.22-M standard (3 passes)
    Dod,
    /// Gutmann method (35 passes, paranoid level)
    Gutmann,
}