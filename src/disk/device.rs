use crate::{Result, UsbBootHutError};
use std::path::PathBuf;
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
    
    // Use diskutil to list all disks
    let output = Command::new("diskutil")
        .args(["list"])
        .output()
        .map_err(|e| UsbBootHutError::Device(format!("Failed to run diskutil: {}", e)))?;
        
    if !output.status.success() {
        return Err(UsbBootHutError::Device(
            "Failed to list disks".to_string()
        ));
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    
    // Parse diskutil output to find external disks
    for line in output_str.lines() {
        if line.starts_with("/dev/disk") && line.contains("external") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(device_path) = parts.first() {
                // Get detailed info about this disk
                if let Ok(device) = get_macos_device_info(device_path) {
                    devices.push(device);
                }
            }
        }
    }
    
    Ok(devices)
}

#[cfg(target_os = "macos")]
fn get_macos_device_info(device_path: &str) -> Result<UsbDevice> {
    use std::process::Command;
    
    // Get device info using diskutil
    let output = Command::new("diskutil")
        .args(["info", device_path])
        .output()
        .map_err(|e| UsbBootHutError::Device(format!("Failed to get device info: {}", e)))?;
        
    if !output.status.success() {
        return Err(UsbBootHutError::Device(
            format!("Failed to get info for {}", device_path)
        ));
    }
    
    let info = String::from_utf8_lossy(&output.stdout);
    let mut size = 0u64;
    let mut removable = false;
    let mut model = "Unknown".to_string();
    let mut vendor = "Unknown".to_string();
    
    // Parse the diskutil info output
    for line in info.lines() {
        let line = line.trim();
        if line.starts_with("Disk Size:") {
            // Extract size in bytes
            if let Some(size_str) = line.split('(').nth(1) {
                if let Some(bytes_str) = size_str.split(" Bytes").next() {
                    size = bytes_str.trim().parse().unwrap_or(0);
                }
            }
        } else if line.starts_with("Removable Media:") {
            removable = line.contains("Yes") || line.contains("Removable");
        } else if line.starts_with("Device / Media Name:") {
            model = line.split(':').nth(1).unwrap_or("").trim().to_string();
        } else if line.starts_with("Protocol:") {
            let protocol = line.split(':').nth(1).unwrap_or("").trim();
            if protocol.contains("USB") {
                vendor = "USB Device".to_string();
            }
        }
    }
    
    // Get partitions
    let partitions = enumerate_macos_partitions(device_path)?;
    
    Ok(UsbDevice {
        path: PathBuf::from(device_path),
        name: device_path.trim_start_matches("/dev/").to_string(),
        size,
        model,
        vendor,
        removable,
        partitions,
    })
}

#[cfg(target_os = "macos")]
fn enumerate_macos_partitions(device_path: &str) -> Result<Vec<Partition>> {
    use std::process::Command;
    
    let output = Command::new("diskutil")
        .args(["list", device_path])
        .output()
        .map_err(|e| UsbBootHutError::Device(format!("Failed to list partitions: {}", e)))?;
        
    if !output.status.success() {
        return Ok(Vec::new()); // Device might not have partitions
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut partitions = Vec::new();
    let mut in_partition_section = false;
    
    for line in output_str.lines() {
        let line = line.trim();
        
        // Look for the partition table section
        if line.contains("IDENTIFIER") {
            in_partition_section = true;
            continue;
        }
        
        if in_partition_section && !line.is_empty() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                // Format: #: TYPE NAME SIZE IDENTIFIER
                if let Ok(number) = parts[0].trim_end_matches(':').parse::<u32>() {
                    let filesystem = parts[1].to_string();
                    let identifier = parts.last().unwrap();
                    
                    // Parse size
                    let size_str = parts[parts.len() - 2];
                    let size = parse_size_string(size_str).unwrap_or(0);
                    
                    partitions.push(Partition {
                        path: PathBuf::from(format!("/dev/{}", identifier)),
                        number,
                        size,
                        filesystem: Some(filesystem),
                        label: None,
                        uuid: None,
                    });
                }
            }
        }
    }
    
    Ok(partitions)
}

#[cfg(target_os = "macos")]
fn parse_size_string(size_str: &str) -> Option<u64> {
    let size_str = size_str.trim();
    
    if size_str.ends_with("GB") {
        let num = size_str.trim_end_matches("GB").parse::<f64>().ok()?;
        Some((num * 1_000_000_000.0) as u64)
    } else if size_str.ends_with("MB") {
        let num = size_str.trim_end_matches("MB").parse::<f64>().ok()?;
        Some((num * 1_000_000.0) as u64)
    } else if size_str.ends_with("KB") {
        let num = size_str.trim_end_matches("KB").parse::<f64>().ok()?;
        Some((num * 1_000.0) as u64)
    } else {
        None
    }
}