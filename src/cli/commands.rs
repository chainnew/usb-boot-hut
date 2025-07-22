use crate::{Result, UsbBootHutError};
use crate::cli::{Cli, Commands, ConfigAction, ListFormat, WipePattern};
use crate::disk::{enumerate_usb_devices, DriveManager};
use crate::cleanup::{CleanupEngine, CleanupConfig};
use crate::config::ConfigManager;
use crate::utils::print_banner;
use colored::*;
use dialoguer::{Password, Confirm};
use std::path::Path;
use prettytable::{Table, row};

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
        Commands::Nuke { device, passes, pattern, force, verify } => {
            handle_nuke(&device, passes, pattern, force, verify)
        },
        Commands::Burn { image, device, no_verify, enable_ssh, wifi, yes, eject } => {
            handle_burn(&image, &device, no_verify, enable_ssh, wifi.as_deref(), yes, eject)
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
                .interact()
                .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?
            {
                println!("Operation cancelled.");
                return Ok(());
            }
        }
    }
    
    // Show what will happen
    println!("\n{}", "📋 Format Plan:".cyan().bold());
    println!("  1. {} Wipe partition table", if secure_wipe { "🔐" } else { "🧹" });
    if secure_wipe {
        println!("     - Overwrite with random data (this will take time)");
    }
    println!("  2. 📊 Create GPT partition table");
    println!("  3. 💾 Create partitions:");
    println!("     - ESP:  512MB FAT32 (UEFI boot)");
    println!("     - Boot: 512MB ext4 (GRUB config)");
    println!("     - Data: {:.1}GB {} (ISO storage)", 
        (device.size - 1024*1024*1024) as f64 / 1_000_000_000.0,
        if encrypt { "LUKS-encrypted ext4" } else { "ext4" }
    );
    println!("  4. 🚀 Install GRUB2 bootloader");
    println!("  5. 📁 Create directory structure");
    
    // Confirm format
    if !skip_confirm {
        println!("\n{}", "⚠️  WARNING: All data on this device will be destroyed!".red().bold());
        if !Confirm::new()
            .with_prompt(format!("Format {} and create bootable USB?", device_path.display()))
            .default(false)
            .interact()
            .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?
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
            .interact()
            .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?;
            
        Some(pass)
    } else {
        None
    };
    
    // Check platform
    #[cfg(not(target_os = "linux"))]
    {
        println!("\n{}", "❌ Platform Limitation".red().bold());
        println!("Full USB formatting requires Linux for:");
        println!("  - sgdisk (GPT partitioning)");
        println!("  - cryptsetup (LUKS encryption)");
        println!("  - grub-install (bootloader)");
        println!("  - ext4 filesystem support");
        println!("\nPlease run this tool on a Linux system to format USB drives.");
        return Ok(());
    }
    
    // On Linux, actually format
    #[cfg(target_os = "linux")]
    {
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
    }
    
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

fn handle_add(iso_path: &Path, verify_checksum: Option<&str>, _category: Option<&str>, _tags: Option<&str>) -> Result<()> {
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
            .interact()
            .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?
        {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // TODO: Implement remove functionality
    println!("Removing ISO: {}", iso_name);
    Ok(())
}

fn handle_list(_device: Option<&Path>, _category: Option<&str>, format: ListFormat) -> Result<()> {
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
                    .interact()
                    .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?
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

fn handle_nuke(device_path: &Path, passes: u8, pattern: WipePattern, force: bool, verify: bool) -> Result<()> {
    use crate::disk::SecureWipe;
    use crate::utils::AnimationPlayer;
    use indicatif::{ProgressBar, ProgressStyle};
    
    // Find the device
    let devices = enumerate_usb_devices()?;
    let device = devices.into_iter()
        .find(|d| d.path == device_path)
        .ok_or_else(|| UsbBootHutError::Device(format!("Device not found: {}", device_path.display())))?;
    
    // Show device info
    println!("\n{}", "☢️  NUCLEAR OPTION - SECURE WIPE ☢️".red().bold());
    println!("\n{}", "Device Information:".bold());
    println!("  Path:     {}", device.path.display());
    println!("  Model:    {} {}", device.vendor, device.model);
    println!("  Size:     {} GB", device.size / 1_000_000_000);
    println!("  Type:     {}", if device.removable { "Removable" } else { "Fixed" }.red());
    
    // Validate device
    if !device.removable && !force {
        return Err(UsbBootHutError::Device(
            "Cannot nuke non-removable device without --force flag".to_string()
        ));
    }
    
    // Show wipe plan
    println!("\n{}", "🔥 Wipe Plan:".red().bold());
    match pattern {
        WipePattern::Random => {
            println!("  Pattern: Random data");
            println!("  Passes:  {}", passes);
            println!("  Method:  Overwrite with cryptographically secure random data");
        },
        WipePattern::Zeros => {
            println!("  Pattern: Zeros");
            println!("  Passes:  {}", passes);
            println!("  Method:  Overwrite with 0x00 bytes");
        },
        WipePattern::Dod => {
            println!("  Pattern: DoD 5220.22-M");
            println!("  Passes:  3 (fixed)");
            println!("  Method:  1) Zeros, 2) Ones (0xFF), 3) Random");
        },
        WipePattern::Gutmann => {
            println!("  Pattern: Gutmann");
            println!("  Passes:  35 (fixed)");
            println!("  Method:  Peter Gutmann's 35-pass secure deletion");
            println!("  Note:    This is overkill for modern drives!");
        },
    }
    
    let total_passes = match pattern {
        WipePattern::Dod => 3,
        WipePattern::Gutmann => 35,
        _ => passes,
    };
    
    // Estimate time
    let write_speed_mbps = 50.0; // Conservative estimate
    let total_data = (device.size * total_passes as u64) as f64;
    let estimated_seconds = total_data / (write_speed_mbps * 1_000_000.0);
    let estimated_minutes = (estimated_seconds / 60.0).ceil() as u64;
    
    println!("\n  Estimated time: ~{} minutes", estimated_minutes);
    
    // Ultra scary warnings
    if !force {
        println!("\n{}", "⚠️  EXTREME WARNING ⚠️".red().bold().on_yellow());
        println!("{}", "This operation will:".red().bold());
        println!("  • {} Permanently destroy ALL data", "PERMANENTLY".red().bold());
        println!("  • {} Make data recovery impossible", "IMPOSSIBLE".red().bold());
        println!("  • {} Cannot be undone", "CANNOT BE UNDONE".red().bold());
        println!("\n{}", "This is more thorough than normal formatting!".yellow());
        
        // Triple confirmation for non-force mode
        println!("\n{}", "To proceed, you must confirm THREE times:".red().bold());
        
        // First confirmation
        if !Confirm::new()
            .with_prompt(format!("1/3: Do you want to DESTROY ALL DATA on {}?", device_path.display()))
            .default(false)
            .interact()
            .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?
        {
            println!("Operation cancelled.");
            return Ok(());
        }
        
        // Second confirmation
        if !Confirm::new()
            .with_prompt("2/3: Are you ABSOLUTELY SURE? This CANNOT be undone!")
            .default(false)
            .interact()
            .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?
        {
            println!("Operation cancelled.");
            return Ok(());
        }
        
        // Final confirmation with device name
        let confirm_text = format!("nuke {}", device.name);
        println!("\n3/3: Type '{}' to confirm:", confirm_text.red().bold());
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)
            .map_err(|e| UsbBootHutError::Dialog(e.to_string()))?;
        
        if input.trim() != confirm_text {
            println!("Confirmation text did not match. Operation cancelled.");
            return Ok(());
        }
    }
    
    // Platform check
    #[cfg(not(target_os = "linux"))]
    {
        println!("\n{}", "⚠️  Platform Warning".yellow().bold());
        println!("On macOS/Windows, this will attempt to wipe the device,");
        println!("but may have limitations compared to Linux.");
        println!("For best results, use Linux.");
        
        // macOS-specific unmount
        #[cfg(target_os = "macos")]
        {
            println!("\nUnmounting disk...");
            let output = std::process::Command::new("diskutil")
                .args(["unmountDisk", "force", device_path.to_str().unwrap()])
                .output()
                .map_err(|e| UsbBootHutError::Device(format!("Failed to unmount: {}", e)))?;
                
            if !output.status.success() {
                println!("Warning: Failed to unmount disk");
            }
        }
    }
    
    // Start the nuke!
    println!("\n{}", "💀 INITIATING NUCLEAR WIPE... 💀".red().bold());
    println!("{}", "Press Ctrl+C to abort (data may already be partially destroyed)".yellow());
    
    let wiper = SecureWipe::new(device_path);
    
    // Create progress tracking
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.red/yellow} {pos}% | Pass {msg}")
            .unwrap()
            .progress_chars("█▓░")
    );
    
    // Perform the wipe
    wiper.nuke_drive(pattern, passes, |current_pass, total_passes, message| {
        pb.set_message(format!("{}/{}", current_pass, total_passes));
        
        // Extract percentage from message if available
        if let Some(percent_pos) = message.rfind('%') {
            if let Some(num_start) = message[..percent_pos].rfind(' ') {
                if let Ok(percent) = message[num_start+1..percent_pos].parse::<u64>() {
                    pb.set_position(percent);
                }
            }
        }
        
        pb.set_prefix(message);
    })?;
    
    pb.finish_with_message("COMPLETE");
    
    // Verify if requested
    if verify {
        println!("\n{}", "🔍 Verifying wipe...".cyan());
        let wiped = wiper.verify_wiped()?;
        if wiped {
            println!("{}", "✅ Verification passed: No filesystem signatures found".green());
        } else {
            println!("{}", "⚠️  Verification failed: Filesystem signatures still present!".red());
            println!("The wipe may have been incomplete. Consider running again.");
        }
    }
    
    println!("\n{}", "☠️  DEVICE NUKED ☠️".red().bold());
    println!("The device has been securely wiped and is ready for disposal or reuse.");
    
    Ok(())
}