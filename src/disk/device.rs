use crate::{Result, UsbBootHutError};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub model: String,
    pub vendor: String,
    pub removable: bool,
    pub partitions: Vec<Partition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub path: PathBuf,
    pub number: u32,
    pub size: u64,
    pub filesystem: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
}

impl UsbDevice {
    pub fn is_valid_for_boot(&self) -> Result<()> {
        if !self.removable {
            return Err(UsbBootHutError::Device(
                "Device is not removable".to_string()
            ));
        }
        
        if self.size < crate::MIN_DRIVE_SIZE {
            return Err(UsbBootHutError::Device(
                format!("Device too small: {} bytes (minimum: {} bytes)", 
                    self.size, crate::MIN_DRIVE_SIZE)
            ));
        }
        
        Ok(())
    }
    
    pub fn has_system_files(&self) -> bool {
        // Check for signs this might be a system drive
        for partition in &self.partitions {
            if let Some(label) = &partition.label {
                let label_lower = label.to_lowercase();
                if label_lower.contains("system") || 
                   label_lower.contains("windows") ||
                   label_lower.contains("recovery") ||
                   label_lower == "efi" {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(target_os = "linux")]
pub fn enumerate_usb_devices() -> Result<Vec<UsbDevice>> {
    use std::fs;
    use std::process::Command;
    
    let mut devices = Vec::new();
    
    // Read block devices from /sys/block
    let block_dir = Path::new("/sys/block");
    for entry in fs::read_dir(block_dir)
        .map_err(|e| UsbBootHutError::Device(format!("Failed to read /sys/block: {}", e)))? 
    {
        let entry = entry
            .map_err(|e| UsbBootHutError::Device(format!("Failed to read entry: {}", e)))?;
        let device_name = entry.file_name();
        let device_name_str = device_name.to_string_lossy();
        
        // Skip non-disk devices
        if device_name_str.starts_with("loop") || 
           device_name_str.starts_with("ram") ||
           device_name_str.starts_with("dm-") {
            continue;
        }
        
        let device_path = PathBuf::from(format!("/dev/{}", device_name_str));
        let sys_path = entry.path();
        
        // Check if removable
        let removable_path = sys_path.join("removable");
        let removable = fs::read_to_string(&removable_path)
            .unwrap_or_default()
            .trim() == "1";
            
        if !removable {
            continue; // Only interested in removable devices
        }
        
        // Get device info
        let size = fs::read_to_string(sys_path.join("size"))
            .unwrap_or_default()
            .trim()
            .parse::<u64>()
            .unwrap_or(0) * 512; // Convert sectors to bytes
            
        let model = fs::read_to_string(sys_path.join("device/model"))
            .unwrap_or_else(|_| "Unknown".to_string())
            .trim()
            .to_string();
            
        let vendor = fs::read_to_string(sys_path.join("device/vendor"))
            .unwrap_or_else(|_| "Unknown".to_string())
            .trim()
            .to_string();
        
        // Get partitions
        let partitions = enumerate_partitions(&device_path)?;
        
        devices.push(UsbDevice {
            path: device_path,
            name: device_name_str.to_string(),
            size,
            model,
            vendor,
            removable,
            partitions,
        });
    }
    
    Ok(devices)
}

#[cfg(target_os = "linux")]
fn enumerate_partitions(device_path: &Path) -> Result<Vec<Partition>> {
    use std::process::Command;
    
    let output = Command::new("lsblk")
        .args([
            "-J", // JSON output
            "-b", // Bytes
            "-o", "NAME,SIZE,FSTYPE,LABEL,UUID,TYPE",
            device_path.to_str().unwrap()
        ])
        .output()
        .map_err(|e| UsbBootHutError::Device(format!("Failed to run lsblk: {}", e)))?;
        
    if !output.status.success() {
        return Ok(Vec::new()); // Device might not have partitions yet
    }
    
    // Parse lsblk JSON output
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| UsbBootHutError::Device(format!("Failed to parse lsblk output: {}", e)))?;
        
    let mut partitions = Vec::new();
    
    if let Some(devices) = json["blockdevices"].as_array() {
        for device in devices {
            if let Some(children) = device["children"].as_array() {
                for (idx, child) in children.iter().enumerate() {
                    if child["type"].as_str() == Some("part") {
                        let name = child["name"].as_str().unwrap_or("");
                        partitions.push(Partition {
                            path: PathBuf::from(format!("/dev/{}", name)),
                            number: (idx + 1) as u32,
                            size: child["size"].as_str()
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(0),
                            filesystem: child["fstype"].as_str().map(String::from),
                            label: child["label"].as_str().map(String::from),
                            uuid: child["uuid"].as_str().map(String::from),
                        });
                    }
                }
            }
        }
    }
    
    Ok(partitions)
}

#[cfg(target_os = "windows")]
pub fn enumerate_usb_devices() -> Result<Vec<UsbDevice>> {
    // Windows implementation using WMI
    use std::process::Command;
    
    let output = Command::new("wmic")
        .args(["diskdrive", "where", "InterfaceType='USB'", "get", 
               "DeviceID,Size,Model,Caption", "/format:csv"])
        .output()
        .map_err(|e| UsbBootHutError::Device(format!("Failed to run wmic: {}", e)))?;
        
    // Parse WMI output
    // Implementation details...
    todo!("Windows USB enumeration")
}

#[cfg(target_os = "macos")]
pub fn enumerate_usb_devices() -> Result<Vec<UsbDevice>> {
    use std::process::Command;
    
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| UsbBootHutError::Device(format!("Failed to run diskutil: {}", e)))?;
        
    // Parse diskutil plist output
    // Implementation details...
    todo!("macOS USB enumeration")
}