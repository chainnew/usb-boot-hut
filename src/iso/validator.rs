use crate::{Result, UsbBootHutError};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use sha2::{Sha256, Digest};

pub struct IsoValidator;

impl IsoValidator {
    pub fn validate_iso(iso_path: &Path) -> Result<IsoInfo> {
        let mut file = File::open(iso_path)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to open ISO: {}", e)))?;
            
        // Check ISO 9660 signature
        let mut buffer = vec![0u8; 6];
        file.seek(SeekFrom::Start(0x8001))
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to seek: {}", e)))?;
        file.read_exact(&mut buffer)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to read: {}", e)))?;
            
        if &buffer != b"CD001\x01" {
            return Err(UsbBootHutError::Iso("Invalid ISO 9660 format".to_string()));
        }
        
        // Get volume ID
        let mut volume_id = vec![0u8; 32];
        file.seek(SeekFrom::Start(0x8028))
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to seek: {}", e)))?;
        file.read_exact(&mut volume_id)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to read volume ID: {}", e)))?;
            
        let volume_id = String::from_utf8_lossy(&volume_id)
            .trim()
            .to_string();
            
        // Check for bootability
        let bootable = Self::check_bootable(&mut file)?;
        
        // Get file size
        file.seek(SeekFrom::End(0))
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to get size: {}", e)))?;
        let size = file.stream_position()
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to get position: {}", e)))?;
            
        Ok(IsoInfo {
            path: iso_path.to_path_buf(),
            volume_id: volume_id.clone(),
            size,
            bootable,
            iso_type: Self::detect_iso_type(&volume_id),
        })
    }
    
    fn check_bootable(file: &mut File) -> Result<bool> {
        // Check El Torito boot record
        let mut buffer = vec![0u8; 32];
        file.seek(SeekFrom::Start(0x8801))
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to seek: {}", e)))?;
        file.read_exact(&mut buffer)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to read boot record: {}", e)))?;
            
        // Check for El Torito signature
        if &buffer[0..5] == b"\x00CD001" && &buffer[30..32] == b"\x55\xAA" {
            return Ok(true);
        }
        
        // Check for UEFI boot
        // This would involve looking for EFI/BOOT/BOOTX64.EFI in the ISO
        // For now, we'll assume ISOs with certain patterns are bootable
        
        Ok(false)
    }
    
    fn detect_iso_type(volume_id: &str) -> IsoType {
        let volume_lower = volume_id.to_lowercase();
        
        if volume_lower.contains("ubuntu") {
            IsoType::Ubuntu
        } else if volume_lower.contains("debian") {
            IsoType::Debian
        } else if volume_lower.contains("arch") {
            IsoType::Arch
        } else if volume_lower.contains("fedora") {
            IsoType::Fedora
        } else if volume_lower.contains("windows") {
            IsoType::Windows
        } else if volume_lower.contains("centos") || volume_lower.contains("rhel") {
            IsoType::RedHat
        } else {
            IsoType::Unknown
        }
    }
    
    pub fn calculate_checksum(iso_path: &Path) -> Result<String> {
        let mut file = File::open(iso_path)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to open ISO: {}", e)))?;
            
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB chunks
        
        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to read: {}", e)))?;
                
            if bytes_read == 0 {
                break;
            }
            
            hasher.update(&buffer[..bytes_read]);
        }
        
        Ok(hex::encode(hasher.finalize()))
    }
    
    pub fn verify_checksum(iso_path: &Path, expected_checksum: &str) -> Result<bool> {
        let calculated = Self::calculate_checksum(iso_path)?;
        Ok(calculated.eq_ignore_ascii_case(expected_checksum))
    }
}

#[derive(Debug, Clone)]
pub struct IsoInfo {
    pub path: std::path::PathBuf,
    pub volume_id: String,
    pub size: u64,
    pub bootable: bool,
    pub iso_type: IsoType,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IsoType {
    Ubuntu,
    Debian,
    Arch,
    Fedora,
    Windows,
    RedHat,
    Unknown,
}