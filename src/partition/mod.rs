use crate::{Result, UsbBootHutError, ESP_SIZE, BOOT_SIZE};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PartitionManager {
    device_path: PathBuf,
}

impl PartitionManager {
    pub fn new(device_path: &Path) -> Self {
        Self {
            device_path: device_path.to_path_buf(),
        }
    }
    
    pub fn wipe_partition_table(&self) -> Result<()> {
        // Use sgdisk to zap all partition data
        let output = Command::new("sgdisk")
            .args(["--zap-all", self.device_path.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to run sgdisk: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to wipe partitions: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        // Also clear MBR
        let output = Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", self.device_path.display()),
                "bs=512",
                "count=1",
            ])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to clear MBR: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to clear MBR: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    pub fn create_gpt(&self) -> Result<()> {
        let output = Command::new("sgdisk")
            .args([
                "--clear",
                "--new=0:0:0", // Create new GPT
                self.device_path.to_str().unwrap()
            ])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to create GPT: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to create GPT: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        // Clear any existing partitions
        let output = Command::new("sgdisk")
            .args(["--zap-all", self.device_path.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to clear partitions: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to clear partitions: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    pub fn create_esp_partition(&self) -> Result<PathBuf> {
        // Create ESP partition (partition 1)
        let esp_size_mb = ESP_SIZE / (1024 * 1024);
        
        let output = Command::new("sgdisk")
            .args([
                &format!("--new=1:0:+{}M", esp_size_mb),
                "--typecode=1:EF00", // EFI System Partition
                "--change-name=1:EFI System Partition",
                self.device_path.to_str().unwrap()
            ])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to create ESP: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to create ESP: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        // Return partition path
        Ok(self.get_partition_path(1))
    }
    
    pub fn create_boot_partition(&self) -> Result<PathBuf> {
        // Create boot partition (partition 2)
        let boot_size_mb = BOOT_SIZE / (1024 * 1024);
        
        let output = Command::new("sgdisk")
            .args([
                &format!("--new=2:0:+{}M", boot_size_mb),
                "--typecode=2:8300", // Linux filesystem
                "--change-name=2:Boot Partition",
                self.device_path.to_str().unwrap()
            ])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to create boot partition: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to create boot partition: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(self.get_partition_path(2))
    }
    
    pub fn create_data_partition(&self) -> Result<PathBuf> {
        // Create data partition (partition 3) using remaining space
        let output = Command::new("sgdisk")
            .args([
                "--new=3:0:0", // Use all remaining space
                "--typecode=3:8300", // Linux filesystem
                "--change-name=3:Data Partition",
                self.device_path.to_str().unwrap()
            ])
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to create data partition: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Partition(
                format!("Failed to create data partition: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        // Inform kernel of partition changes
        self.refresh_partitions()?;
        
        Ok(self.get_partition_path(3))
    }
    
    fn get_partition_path(&self, number: u32) -> PathBuf {
        let device_str = self.device_path.to_str().unwrap();
        
        // Handle different partition naming schemes
        if device_str.contains("nvme") || device_str.contains("mmcblk") {
            // NVMe and MMC devices use 'p' before partition number
            PathBuf::from(format!("{}p{}", device_str, number))
        } else {
            // Regular devices like /dev/sda just append the number
            PathBuf::from(format!("{}{}", device_str, number))
        }
    }
    
    fn refresh_partitions(&self) -> Result<()> {
        // Tell kernel to re-read partition table
        let output = Command::new("partprobe")
            .arg(self.device_path.to_str().unwrap())
            .output()
            .map_err(|e| UsbBootHutError::Partition(format!("Failed to run partprobe: {}", e)))?;
            
        if !output.status.success() {
            // Try alternative method
            let output = Command::new("blockdev")
                .args(["--rereadpt", self.device_path.to_str().unwrap()])
                .output()
                .map_err(|e| UsbBootHutError::Partition(format!("Failed to refresh partitions: {}", e)))?;
                
            if !output.status.success() {
                return Err(UsbBootHutError::Partition(
                    "Failed to refresh partition table".to_string()
                ));
            }
        }
        
        // Give kernel a moment to update device nodes
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        Ok(())
    }
    
    pub fn verify_partitions(&self) -> Result<()> {
        // Verify all expected partitions exist
        for i in 1..=3 {
            let part_path = self.get_partition_path(i);
            if !part_path.exists() {
                return Err(UsbBootHutError::Partition(
                    format!("Partition {} does not exist at {}", i, part_path.display())
                ));
            }
        }
        
        Ok(())
    }
}