use crate::{Result, UsbBootHutError};
use std::fs::{File, OpenOptions};
use std::io::{self, Write, Seek, SeekFrom};
use std::path::Path;

pub struct SecureWipe<'a> {
    device_path: &'a Path,
}

impl<'a> SecureWipe<'a> {
    pub fn new(device_path: &'a Path) -> Self {
        Self { device_path }
    }
    
    pub fn quick_wipe(&self) -> Result<()> {
        // Just wipe the first and last 1MB to destroy partition tables
        let mut file = OpenOptions::new()
            .write(true)
            .open(self.device_path)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to open device: {}", e)))?;
            
        // Wipe first 1MB
        let zeros = vec![0u8; 1024 * 1024];
        file.write_all(&zeros)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to wipe start: {}", e)))?;
            
        // Get device size and wipe last 1MB
        let size = file.seek(SeekFrom::End(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to seek: {}", e)))?;
            
        if size > 1024 * 1024 {
            file.seek(SeekFrom::Start(size - 1024 * 1024))
                .map_err(|e| UsbBootHutError::Device(format!("Failed to seek to end: {}", e)))?;
            file.write_all(&zeros)
                .map_err(|e| UsbBootHutError::Device(format!("Failed to wipe end: {}", e)))?;
        }
        
        file.sync_all()
            .map_err(|e| UsbBootHutError::Device(format!("Failed to sync: {}", e)))?;
            
        Ok(())
    }
    
    pub fn wipe_with_progress<F>(&self, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u8), // Progress from 0-100
    {
        let mut file = OpenOptions::new()
            .write(true)
            .open(self.device_path)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to open device: {}", e)))?;
            
        // Get device size
        let size = file.seek(SeekFrom::End(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to get size: {}", e)))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to seek: {}", e)))?;
            
        // Use 4MB chunks for better performance
        const CHUNK_SIZE: usize = 4 * 1024 * 1024;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        
        // Fill buffer with random data
        use std::fs::File;
        use std::io::Read;
        
        let mut urandom = File::open("/dev/urandom")
            .map_err(|e| UsbBootHutError::Device(format!("Failed to open /dev/urandom: {}", e)))?;
            
        let mut written = 0u64;
        
        while written < size {
            // Read random data
            urandom.read_exact(&mut buffer)
                .map_err(|e| UsbBootHutError::Device(format!("Failed to read random: {}", e)))?;
                
            // Write to device
            let to_write = std::cmp::min(CHUNK_SIZE, (size - written) as usize);
            file.write_all(&buffer[..to_write])
                .map_err(|e| UsbBootHutError::Device(format!("Failed to write: {}", e)))?;
                
            written += to_write as u64;
            
            // Update progress
            let progress = ((written as f64 / size as f64) * 100.0) as u8;
            progress_callback(progress);
        }
        
        file.sync_all()
            .map_err(|e| UsbBootHutError::Device(format!("Failed to sync: {}", e)))?;
            
        Ok(())
    }
    
    pub fn verify_wiped(&self) -> Result<bool> {
        // Read first 1MB and check if it's all zeros or random
        let mut file = File::open(self.device_path)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to open device: {}", e)))?;
            
        let mut buffer = vec![0u8; 1024 * 1024];
        use std::io::Read;
        file.read_exact(&mut buffer)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to read: {}", e)))?;
            
        // Check for common partition signatures
        let signatures = [
            &b"EFI PART"[..], // GPT
            &b"\x55\xAA"[..], // MBR boot signature at offset 510
            &b"NTFS"[..],     // NTFS
            &b"FAT32"[..],    // FAT32
            &b"\x53\xEF"[..], // ext2/3/4 at offset 0x438
        ];
        
        for sig in &signatures {
            if buffer.windows(sig.len()).any(|w| w == *sig) {
                return Ok(false); // Found a signature, not wiped
            }
        }
        
        Ok(true)
    }
}