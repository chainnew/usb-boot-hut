use crate::{Result, UsbBootHutError};
use crate::disk::{UsbDevice, SecureWipe};
use crate::partition::PartitionManager;
use crate::crypto::LuksManager;
use std::path::{Path, PathBuf};
use indicatif::{ProgressBar, ProgressStyle};

pub struct DriveManager {
    device: UsbDevice,
    encryption_enabled: bool,
}

impl DriveManager {
    pub fn new(device: UsbDevice) -> Self {
        Self {
            device,
            encryption_enabled: false,
        }
    }
    
    pub fn with_encryption(mut self) -> Self {
        self.encryption_enabled = true;
        self
    }
    
    pub fn format_and_setup(&self, passphrase: Option<&str>) -> Result<()> {
        // Validate device
        self.device.is_valid_for_boot()?;
        
        // Safety check
        if self.device.has_system_files() {
            return Err(UsbBootHutError::Device(
                "Device appears to contain system files. Please confirm this is the correct device.".to_string()
            ));
        }
        
        println!("Preparing to format device: {}", self.device.path.display());
        println!("Model: {} {}", self.device.vendor, self.device.model);
        println!("Size: {} GB", self.device.size / 1_000_000_000);
        
        // Create partition manager
        let partition_mgr = PartitionManager::new(&self.device.path);
        
        // Step 1: Wipe partition table
        println!("\n[1/6] Wiping partition table...");
        partition_mgr.wipe_partition_table()?;
        
        // Step 2: Create GPT
        println!("[2/6] Creating GPT partition table...");
        partition_mgr.create_gpt()?;
        
        // Step 3: Create partitions
        println!("[3/6] Creating partitions...");
        let esp_part = partition_mgr.create_esp_partition()?;
        let boot_part = partition_mgr.create_boot_partition()?;
        let data_part = partition_mgr.create_data_partition()?;
        
        // Step 4: Format partitions
        println!("[4/6] Formatting partitions...");
        self.format_esp(&esp_part)?;
        self.format_boot(&boot_part)?;
        
        // Step 5: Setup encryption if enabled
        if self.encryption_enabled {
            if let Some(pass) = passphrase {
                println!("[5/6] Setting up LUKS encryption...");
                let luks_mgr = LuksManager::new();
                luks_mgr.create_encrypted_partition(&data_part, pass)?;
                
                // Open the encrypted partition
                let mapped_name = format!("usb_boot_hut_{}", uuid::Uuid::new_v4());
                luks_mgr.open_encrypted_partition(&data_part, pass, &mapped_name)?;
                
                // Format the opened LUKS device
                let mapped_path = PathBuf::from(format!("/dev/mapper/{}", mapped_name));
                self.format_data(&mapped_path)?;
                
                // Close the encrypted partition
                luks_mgr.close_encrypted_partition(&mapped_name)?;
            } else {
                return Err(UsbBootHutError::Encryption(
                    "Passphrase required for encryption".to_string()
                ));
            }
        } else {
            println!("[5/6] Formatting data partition...");
            self.format_data(&data_part)?;
        }
        
        // Step 6: Install bootloader
        println!("[6/6] Installing GRUB bootloader...");
        self.install_grub(&esp_part, &boot_part)?;
        
        println!("\n✓ USB drive successfully formatted and configured!");
        Ok(())
    }
    
    pub fn secure_format(&self, passphrase: Option<&str>) -> Result<()> {
        // First do a secure wipe
        println!("Performing secure wipe (this may take a while)...");
        let wiper = SecureWipe::new(&self.device.path);
        
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}% {msg}")
                .unwrap()
                .progress_chars("##-")
        );
        
        wiper.wipe_with_progress(|progress| {
            pb.set_position(progress as u64);
        })?;
        
        pb.finish_with_message("Secure wipe complete");
        
        // Then format normally
        self.format_and_setup(passphrase)
    }
    
    fn format_esp(&self, partition: &Path) -> Result<()> {
        use std::process::Command;
        
        let output = Command::new("mkfs.fat")
            .args(["-F", "32", "-n", "USB_ESP"])
            .arg(partition)
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to format ESP: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("mkfs.fat failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    fn format_boot(&self, partition: &Path) -> Result<()> {
        use std::process::Command;
        
        let output = Command::new("mkfs.ext4")
            .args(["-L", "USB_BOOT", "-F"])
            .arg(partition)
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to format boot: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("mkfs.ext4 failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    fn format_data(&self, partition: &Path) -> Result<()> {
        use std::process::Command;
        
        let output = Command::new("mkfs.ext4")
            .args(["-L", "USB_DATA", "-F"])
            .arg(partition)
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to format data: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("mkfs.ext4 failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    fn install_grub(&self, esp_partition: &Path, boot_partition: &Path) -> Result<()> {
        use crate::bootloader::GrubInstaller;
        
        let installer = GrubInstaller::new(&self.device.path);
        installer.install(esp_partition, boot_partition)?;
        
        Ok(())
    }
}