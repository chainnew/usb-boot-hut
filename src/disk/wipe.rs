use crate::{Result, UsbBootHutError};
use crate::cli::WipePattern;
use std::fs::{File, OpenOptions};
use std::io::{Write, Seek, SeekFrom, Read};
use std::path::Path;
// use colored::*; // Not needed currently

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
    
    pub fn nuke_drive(&self, pattern: WipePattern, passes: u8, progress_callback: impl Fn(u8, u8, &str)) -> Result<()> {
        match pattern {
            WipePattern::Random => self.nuke_random(passes, progress_callback),
            WipePattern::Zeros => self.nuke_zeros(passes, progress_callback),
            WipePattern::Dod => self.nuke_dod(progress_callback),
            WipePattern::Gutmann => self.nuke_gutmann(progress_callback),
        }
    }
    
    fn nuke_random(&self, passes: u8, progress_callback: impl Fn(u8, u8, &str)) -> Result<()> {
        for pass in 1..=passes {
            progress_callback(pass, passes, &format!("Pass {}/{}: Writing random data", pass, passes));
            self.wipe_with_random(|percent| {
                progress_callback(pass, passes, &format!("Pass {}/{}: {}%", pass, passes, percent));
            })?;
        }
        Ok(())
    }
    
    fn nuke_zeros(&self, passes: u8, progress_callback: impl Fn(u8, u8, &str)) -> Result<()> {
        for pass in 1..=passes {
            progress_callback(pass, passes, &format!("Pass {}/{}: Writing zeros", pass, passes));
            self.wipe_with_zeros(|percent| {
                progress_callback(pass, passes, &format!("Pass {}/{}: {}%", pass, passes, percent));
            })?;
        }
        Ok(())
    }
    
    fn nuke_dod(&self, progress_callback: impl Fn(u8, u8, &str)) -> Result<()> {
        // DoD 5220.22-M: 3 passes
        // Pass 1: Write zeros
        progress_callback(1, 3, "Pass 1/3: Writing zeros (DoD 5220.22-M)");
        self.wipe_with_zeros(|percent| {
            progress_callback(1, 3, &format!("Pass 1/3: {}%", percent));
        })?;
        
        // Pass 2: Write ones (0xFF)
        progress_callback(2, 3, "Pass 2/3: Writing ones (DoD 5220.22-M)");
        self.wipe_with_pattern(0xFF, |percent| {
            progress_callback(2, 3, &format!("Pass 2/3: {}%", percent));
        })?;
        
        // Pass 3: Write random
        progress_callback(3, 3, "Pass 3/3: Writing random data (DoD 5220.22-M)");
        self.wipe_with_random(|percent| {
            progress_callback(3, 3, &format!("Pass 3/3: {}%", percent));
        })?;
        
        Ok(())
    }
    
    fn nuke_gutmann(&self, progress_callback: impl Fn(u8, u8, &str)) -> Result<()> {
        // Gutmann method: 35 passes with specific patterns
        let patterns: Vec<Vec<u8>> = vec![
            // Random passes
            vec![0; 0], vec![0; 0], vec![0; 0], vec![0; 0], // Passes 1-4: random
            // Fixed patterns
            vec![0x55, 0x55, 0x55], // Pass 5
            vec![0xAA, 0xAA, 0xAA], // Pass 6
            vec![0x92, 0x49, 0x24], // Pass 7
            vec![0x49, 0x24, 0x92], // Pass 8
            vec![0x24, 0x92, 0x49], // Pass 9
            vec![0x00, 0x00, 0x00], // Pass 10
            vec![0x11, 0x11, 0x11], // Pass 11
            vec![0x22, 0x22, 0x22], // Pass 12
            vec![0x33, 0x33, 0x33], // Pass 13
            vec![0x44, 0x44, 0x44], // Pass 14
            vec![0x55, 0x55, 0x55], // Pass 15
            vec![0x66, 0x66, 0x66], // Pass 16
            vec![0x77, 0x77, 0x77], // Pass 17
            vec![0x88, 0x88, 0x88], // Pass 18
            vec![0x99, 0x99, 0x99], // Pass 19
            vec![0xAA, 0xAA, 0xAA], // Pass 20
            vec![0xBB, 0xBB, 0xBB], // Pass 21
            vec![0xCC, 0xCC, 0xCC], // Pass 22
            vec![0xDD, 0xDD, 0xDD], // Pass 23
            vec![0xEE, 0xEE, 0xEE], // Pass 24
            vec![0xFF, 0xFF, 0xFF], // Pass 25
            vec![0x92, 0x49, 0x24], // Pass 26
            vec![0x49, 0x24, 0x92], // Pass 27
            vec![0x24, 0x92, 0x49], // Pass 28
            vec![0x6D, 0xB6, 0xDB], // Pass 29
            vec![0xB6, 0xDB, 0x6D], // Pass 30
            vec![0xDB, 0x6D, 0xB6], // Pass 31
            // Final random passes
            vec![0; 0], vec![0; 0], vec![0; 0], vec![0; 0], // Passes 32-35: random
        ];
        
        for (i, pattern) in patterns.iter().enumerate() {
            let pass = (i + 1) as u8;
            
            if pattern.is_empty() {
                // Random pass
                progress_callback(pass, 35, &format!("Pass {}/35: Writing random (Gutmann)", pass));
                self.wipe_with_random(|percent| {
                    progress_callback(pass, 35, &format!("Pass {}/35: {}%", pass, percent));
                })?;
            } else {
                // Pattern pass
                progress_callback(pass, 35, &format!("Pass {}/35: Writing pattern (Gutmann)", pass));
                self.wipe_with_repeating_pattern(pattern, |percent| {
                    progress_callback(pass, 35, &format!("Pass {}/35: {}%", pass, percent));
                })?;
            }
        }
        
        Ok(())
    }
    
    fn wipe_with_zeros<F>(&self, progress_callback: F) -> Result<()>
    where
        F: FnMut(u8),
    {
        self.wipe_with_pattern(0x00, progress_callback)
    }
    
    fn wipe_with_pattern<F>(&self, byte: u8, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u8),
    {
        let mut file = OpenOptions::new()
            .write(true)
            .open(self.device_path)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to open device: {}", e)))?;
            
        let size = file.seek(SeekFrom::End(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to get size: {}", e)))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to seek: {}", e)))?;
            
        const CHUNK_SIZE: usize = 4 * 1024 * 1024;
        let buffer = vec![byte; CHUNK_SIZE];
        let mut written = 0u64;
        
        while written < size {
            let to_write = std::cmp::min(CHUNK_SIZE, (size - written) as usize);
            file.write_all(&buffer[..to_write])
                .map_err(|e| UsbBootHutError::Device(format!("Failed to write: {}", e)))?;
                
            written += to_write as u64;
            let progress = ((written as f64 / size as f64) * 100.0) as u8;
            progress_callback(progress);
        }
        
        file.sync_all()
            .map_err(|e| UsbBootHutError::Device(format!("Failed to sync: {}", e)))?;
            
        Ok(())
    }
    
    fn wipe_with_repeating_pattern<F>(&self, pattern: &[u8], mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u8),
    {
        if pattern.is_empty() {
            return Err(UsbBootHutError::Device("Empty pattern".to_string()));
        }
        
        let mut file = OpenOptions::new()
            .write(true)
            .open(self.device_path)
            .map_err(|e| UsbBootHutError::Device(format!("Failed to open device: {}", e)))?;
            
        let size = file.seek(SeekFrom::End(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to get size: {}", e)))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| UsbBootHutError::Device(format!("Failed to seek: {}", e)))?;
            
        const CHUNK_SIZE: usize = 4 * 1024 * 1024;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        
        // Fill buffer with repeating pattern
        for i in 0..CHUNK_SIZE {
            buffer[i] = pattern[i % pattern.len()];
        }
        
        let mut written = 0u64;
        
        while written < size {
            let to_write = std::cmp::min(CHUNK_SIZE, (size - written) as usize);
            file.write_all(&buffer[..to_write])
                .map_err(|e| UsbBootHutError::Device(format!("Failed to write: {}", e)))?;
                
            written += to_write as u64;
            let progress = ((written as f64 / size as f64) * 100.0) as u8;
            progress_callback(progress);
        }
        
        file.sync_all()
            .map_err(|e| UsbBootHutError::Device(format!("Failed to sync: {}", e)))?;
            
        Ok(())
    }
    
    fn wipe_with_random<F>(&self, progress_callback: F) -> Result<()>
    where
        F: FnMut(u8),
    {
        self.wipe_with_progress(progress_callback)
    }
}