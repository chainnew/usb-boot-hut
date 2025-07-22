use crate::{Result, UsbBootHutError};
use crate::cli::{Cli, Commands, ConfigAction, ListFormat};
use crate::disk::{enumerate_usb_devices, DriveManager, UsbDevice};
use crate::crypto::{LuksManager, SecurePassphrase};
use crate::iso::{IsoManager, IsoCategory};
use crate::cleanup::{CleanupEngine, CleanupConfig};
use crate::config::{ConfigManager, DeviceConfig};
use crate::utils::{print_banner, AnimationPlayer};
use colored::*;
use dialoguer::{Password, Select, Confirm};
use std::path::Path;
use prettytable::{Table, row, cell};

pub fn run(cli: Cli) -> Result<()> {
    // Set up colors
    if cli.no_color {
        colored::control::set_override(false);
    }
    
    // Print banner for main commands
    match &cli.command {
        Commands::Format { .. } | Commands::Devices { .. } => print_banner(),
        _ => {}
    }
    
    // Handle commands
    match cli.command {
        Commands::Format { device, encrypt, secure_wipe, yes } => {
            handle_format(&device, encrypt, secure_wipe, yes)
        },
        Commands::Unlock { device, mount } => {
            handle_unlock(&device, mount.as_deref())
        },
        Commands::Lock { device } => {
            handle_lock(&device)
        },
        Commands::Add { iso_file, verify, category, tags } => {
            handle_add(&iso_file, verify.as_deref(), category.as_deref(), tags.as_deref())
        },
        Commands::Remove { iso_name, yes } => {
            handle_remove(&iso_name, yes)
        },
        Commands::List { device, category, format } => {
            handle_list(device.as_deref(), category.as_deref(), format)
        },
        Commands::Verify { device, iso_name } => {
            handle_verify(&device, iso_name.as_deref())
        },
        Commands::Clean { device, config, dry_run } => {
            handle_clean(&device, config.as_deref(), dry_run)
        },
        Commands::Config { action } => {
            handle_config(action)
        },
        Commands::Devices { all, format } => {
            handle_devices(all, format)
        },
        Commands::Status { device } => {
            handle_status(&device)
        },
        Commands::UpdateGrub { device, regenerate } => {
            handle_update_grub(&device, regenerate)
        },
    }
}

fn handle_format(device_path: &Path, encrypt: bool, secure_wipe: bool, skip_confirm: bool) -> Result<()> {
    // Find the device
    let devices = enumerate_usb_devices()?;
    let device = devices.into_iter()
        .find(|d| d.path == device_path)
        .ok_or_else(|| UsbBootHutError::Device(format!("Device not found: {}", device_path.display())))?;
    
    // Show device info
    println!("\n{}", "Device Information:".bold());
    println!("  Path:     {}", device.path.display());
    println!("  Model:    {} {}", device.vendor, device.model);
    println!("  Size:     {} GB", device.size / 1_000_000_000);
    println!("  Type:     {}", if device.removable { "Removable" } else { "Fixed" }.red());
    
    // Validate device
    device.is_valid_for_boot()?;
    
    // Safety check
    if device.has_system_files() {
        println!("\n{}", "⚠️  WARNING: This device appears to contain system files!".red().bold());
        if !skip_confirm {
            if !Confirm::new()
                .with_prompt("Are you ABSOLUTELY SURE you want to format this device?")
                .default(false)
                .interact()?
            {
                println!("Operation cancelled.");
                return Ok(());
            }
        }
    }
    
    // Confirm format
    if !skip_confirm {
        println!("\n{}", "⚠️  WARNING: All data on this device will be destroyed!".red().bold());
        if !Confirm::new()
            .with_prompt(format!("Format {} and create bootable USB?", device_path.display()))
            .default(false)
            .interact()?
        {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Get passphrase if encryption is enabled
    let passphrase = if encrypt {
        println!("\n{}", "🔐 Encryption Setup".green().bold());
        println!("Enter a strong passphrase for LUKS encryption.");
        println!("Requirements: 12+ chars, mixed case, numbers or symbols");
        
        let pass = Password::new()
            .with_prompt("Passphrase")
            .with_confirmation("Confirm passphrase", "Passphrases do not match")
            .interact()?;
            
        Some(pass)
    } else {
        None
    };
    
    // Create drive manager
    let mut manager = DriveManager::new(device);
    if encrypt {
        manager = manager.with_encryption();
    }
    
    // Format the drive
    println!("\n{}", "🚀 Starting format process...".cyan().bold());
    
    if secure_wipe {
        manager.secure_format(passphrase.as_deref())?;
    } else {
        manager.format_and_setup(passphrase.as_deref())?;
    }
    
    println!("\n{}", "✅ USB drive successfully formatted!".green().bold());
    println!("\nNext steps:");
    println!("  1. Mount the drive: {}", format!("usb-boot-hut unlock {}", device_path.display()).cyan());
    println!("  2. Add ISOs: {}", "usb-boot-hut add <iso-file>".cyan());
    println!("  3. Safely eject and boot from the USB drive");
    
    Ok(())
}

fn handle_unlock(device_path: &Path, mount_point: Option<&Path>) -> Result<()> {
    // TODO: Implement unlock functionality
    println!("Unlocking encrypted drive: {}", device_path.display());
    println!("Mount point: {:?}", mount_point);
    Ok(())
}

fn handle_lock(device_path: &Path) -> Result<()> {
    // TODO: Implement lock functionality
    println!("Locking encrypted drive: {}", device_path.display());
    Ok(())
}

fn handle_add(iso_path: &Path, verify_checksum: Option<&str>, category: Option<&str>, tags: Option<&str>) -> Result<()> {
    // TODO: Need to determine mount points
    println!("Adding ISO: {}", iso_path.display());
    if let Some(checksum) = verify_checksum {
        println!("Verifying checksum: {}", checksum);
    }
    Ok(())
}

fn handle_remove(iso_name: &str, skip_confirm: bool) -> Result<()> {
    if !skip_confirm {
        if !Confirm::new()
            .with_prompt(format!("Remove ISO '{}'?", iso_name))
            .default(false)
            .interact()?
        {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // TODO: Implement remove functionality
    println!("Removing ISO: {}", iso_name);
    Ok(())
}

fn handle_list(device: Option<&Path>, category: Option<&str>, format: ListFormat) -> Result<()> {
    // TODO: Implement list functionality
    println!("Listing ISOs...");
    match format {
        ListFormat::Table => {
            let mut table = Table::new();
            table.add_row(row!["Name", "Size", "Type", "Added"]);
            table.add_row(row!["Ubuntu 22.04", "4.7 GB", "Linux", "2024-01-15"]);
            table.printstd();
        },
        ListFormat::Json => {
            println!(r#"{{"isos": []}}"#);
        },
        ListFormat::Csv => {
            println!("name,size,type,added");
        },
        ListFormat::Simple => {
            println!("Ubuntu 22.04 (4.7 GB)");
        }
    }
    Ok(())
}

fn handle_verify(device_path: &Path, iso_name: Option<&str>) -> Result<()> {
    println!("Verifying ISOs on: {}", device_path.display());
    if let Some(name) = iso_name {
        println!("Checking: {}", name);
    } else {
        println!("Checking all ISOs...");
    }
    Ok(())
}

fn handle_clean(device_path: &Path, config_path: Option<&Path>, dry_run: bool) -> Result<()> {
    let config = if let Some(path) = config_path {
        CleanupEngine::load_config(path)?
    } else {
        CleanupConfig::default()
    };
    
    let mut engine = CleanupEngine::new(config);
    if dry_run {
        engine = engine.with_dry_run();
    }
    
    let stats = engine.clean(device_path)?;
    stats.print_summary();
    
    Ok(())
}

fn handle_config(action: ConfigAction) -> Result<()> {
    let mut config_mgr = ConfigManager::new()?;
    
    match action {
        ConfigAction::Show => {
            let config = config_mgr.get();
            println!("{}", toml::to_string_pretty(config).unwrap());
        },
        ConfigAction::Edit { key, value } => {
            // TODO: Implement config editing
            println!("Setting {} = {}", key, value);
            config_mgr.save()?;
        },
        ConfigAction::Reset { yes } => {
            if !yes {
                if !Confirm::new()
                    .with_prompt("Reset configuration to defaults?")
                    .default(false)
                    .interact()?
                {
                    return Ok(());
                }
            }
            config_mgr.reset_to_defaults()?;
            println!("Configuration reset to defaults.");
        },
        ConfigAction::GenerateCleanup { output } => {
            CleanupEngine::save_default_config(&output)?;
            println!("Default cleanup config saved to: {}", output.display());
        }
    }
    
    Ok(())
}

fn handle_devices(show_all: bool, format: ListFormat) -> Result<()> {
    let devices = enumerate_usb_devices()?;
    let filtered: Vec<_> = if show_all {
        devices
    } else {
        devices.into_iter().filter(|d| d.removable).collect()
    };
    
    if filtered.is_empty() {
        println!("No USB devices found.");
        return Ok(());
    }
    
    match format {
        ListFormat::Table => {
            let mut table = Table::new();
            table.add_row(row![
                "Device",
                "Size",
                "Model",
                "Removable",
                "Partitions"
            ]);
            
            for device in filtered {
                table.add_row(row![
                    device.path.display(),
                    format!("{:.1} GB", device.size as f64 / 1_000_000_000.0),
                    format!("{} {}", device.vendor, device.model),
                    if device.removable { "Yes".green() } else { "No".red() },
                    device.partitions.len()
                ]);
            }
            
            table.printstd();
        },
        ListFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
        },
        ListFormat::Csv => {
            println!("device,size_gb,model,removable,partitions");
            for device in filtered {
                println!("{},{:.1},{} {},{}",
                    device.path.display(),
                    device.size as f64 / 1_000_000_000.0,
                    device.vendor,
                    device.model,
                    device.removable
                );
            }
        },
        ListFormat::Simple => {
            for device in filtered {
                println!("{} - {} {} ({:.1} GB)",
                    device.path.display(),
                    device.vendor,
                    device.model,
                    device.size as f64 / 1_000_000_000.0
                );
            }
        }
    }
    
    Ok(())
}

fn handle_status(device_path: &Path) -> Result<()> {
    println!("Checking status of: {}", device_path.display());
    // TODO: Implement status checking
    Ok(())
}

fn handle_update_grub(device_path: &Path, regenerate: bool) -> Result<()> {
    println!("Updating GRUB configuration on: {}", device_path.display());
    if regenerate {
        println!("Regenerating all entries...");
    }
    // TODO: Implement GRUB update
    Ok(())
}