pub mod cli;
pub mod disk;
pub mod partition;
pub mod crypto;
pub mod bootloader;
pub mod iso;
pub mod cleanup;
pub mod config;
pub mod utils;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum UsbBootHutError {
    #[error("Device error: {0}")]
    Device(String),
    
    #[error("Partition error: {0}")]
    Partition(String),
    
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    #[error("Bootloader error: {0}")]
    Bootloader(String),
    
    #[error("ISO error: {0}")]
    Iso(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Permission error: {0}")]
    Permission(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Dialog error: {0}")]
    Dialog(String),
    
    #[error("Platform not supported: {0}")]
    UnsupportedPlatform(String),
}

pub type Result<T> = std::result::Result<T, UsbBootHutError>;

pub const APP_NAME: &str = "USB Boot Hut";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MIN_DRIVE_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4GB minimum
pub const ESP_SIZE: u64 = 512 * 1024 * 1024; // 512MB
pub const BOOT_SIZE: u64 = 512 * 1024 * 1024; // 512MB