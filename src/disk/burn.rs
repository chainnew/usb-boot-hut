use crate::{Result, UsbBootHutError};
use std::path::{Path, PathBuf};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom, BufReader, BufWriter};
use indicatif::{ProgressBar, ProgressStyle};
use flate2::read::GzDecoder;

pub struct ImageBurner {
    source_path: PathBuf,
    target_device: PathBuf,
    buffer_size: usize,
}

impl ImageBurner {
    pub fn new(source: &Path, target: &Path) -> Self {
        Self {
            source_path: source.to_path_buf(),
            target_device: target.to_path_buf(),
            buffer_size: 4 * 1024 * 1024, // 4MB buffer
        }
    }
    
    pub fn burn(&self) -> Result<()> {
        let source_size = self.get_source_size()?;
        let target_size = self.get_device_size()?;
        
        if source_size > target_size {
            return Err(UsbBootHutError::Device(
                format!("Image too large: {} bytes, device only {} bytes", 
                    source_size, target_size)
            ));
        }
        
        // Create progress bar
        let pb = ProgressBar::new(source_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("█▓▒░ ")
        );
        pb.set_message("Burning image");
        
        // Open source
        let source = File::open(&self.source_path)
            .map_err(|e| UsbBootHutError::Io(e))?;
            
        let mut reader: Box<dyn Read> = if self.source_path.to_string_lossy().ends_with(".gz") {
            Box::new(GzDecoder::new(source))
        } else if self.source_path.to_string_lossy().ends_with(".xz") {
            return Err(UsbBootHutError::Device(
                "XZ decompression not yet implemented. Please decompress the image first.".to_string()
            ));
        } else {
            Box::new(source)
        };
        
        // Open target device
        let mut target = OpenOptions::new()
            .write(true)
            .open(&self.target_device)
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to open device for writing: {}", e)
            ))?;
            
        // Burn the image
        let mut buffer = vec![0u8; self.buffer_size];
        let mut total_written = 0u64;
        
        loop {
            let bytes_read = reader.read(&mut buffer)
                .map_err(|e| UsbBootHutError::Io(e))?;
                
            if bytes_read == 0 {
                break;
            }
            
            target.write_all(&buffer[..bytes_read])
                .map_err(|e| UsbBootHutError::Device(
                    format!("Failed to write to device: {}", e)
                ))?;
                
            total_written += bytes_read as u64;
            pb.set_position(total_written);
        }
        
        // Sync to ensure all data is written
        target.sync_all()
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to sync data: {}", e)
            ))?;
            
        pb.finish_with_message("Image burned successfully");
        
        Ok(())
    }
    
    pub fn verify(&self) -> Result<bool> {
        println!("Verifying burned image...");
        
        let source_size = self.get_source_size()?;
        
        // Create progress bar
        let pb = ProgressBar::new(source_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/red} {bytes}/{total_bytes} ({bytes_per_sec})")
                .unwrap()
                .progress_chars("█▓▒░ ")
        );
        pb.set_message("Verifying");
        
        // Open source
        let source = File::open(&self.source_path)
            .map_err(|e| UsbBootHutError::Io(e))?;
            
        let mut source_reader: Box<dyn Read> = if self.source_path.to_string_lossy().ends_with(".gz") {
            Box::new(GzDecoder::new(source))
        } else {
            Box::new(source)
        };
        
        // Open target
        let mut target = File::open(&self.target_device)
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to open device for verification: {}", e)
            ))?;
            
        // Compare data
        let mut source_buffer = vec![0u8; self.buffer_size];
        let mut target_buffer = vec![0u8; self.buffer_size];
        let mut total_verified = 0u64;
        
        loop {
            let source_bytes = source_reader.read(&mut source_buffer)
                .map_err(|e| UsbBootHutError::Io(e))?;
                
            if source_bytes == 0 {
                break;
            }
            
            let target_bytes = target.read(&mut target_buffer[..source_bytes])
                .map_err(|e| UsbBootHutError::Device(
                    format!("Failed to read from device: {}", e)
                ))?;
                
            if target_bytes != source_bytes {
                pb.abandon();
                return Ok(false);
            }
            
            if source_buffer[..source_bytes] != target_buffer[..source_bytes] {
                pb.abandon();
                return Ok(false);
            }
            
            total_verified += source_bytes as u64;
            pb.set_position(total_verified);
        }
        
        pb.finish_with_message("Verification complete");
        Ok(true)
    }
    
    fn get_source_size(&self) -> Result<u64> {
        let metadata = std::fs::metadata(&self.source_path)
            .map_err(|e| UsbBootHutError::Io(e))?;
            
        if self.source_path.to_string_lossy().ends_with(".gz") {
            // For gzipped files, we need to read the uncompressed size
            // This is stored in the last 4 bytes of the file
            let mut file = File::open(&self.source_path)
                .map_err(|e| UsbBootHutError::Io(e))?;
                
            file.seek(SeekFrom::End(-4))
                .map_err(|e| UsbBootHutError::Io(e))?;
                
            let mut size_bytes = [0u8; 4];
            file.read_exact(&mut size_bytes)
                .map_err(|e| UsbBootHutError::Io(e))?;
                
            Ok(u32::from_le_bytes(size_bytes) as u64)
        } else {
            Ok(metadata.len())
        }
    }
    
    fn get_device_size(&self) -> Result<u64> {
        let mut file = File::open(&self.target_device)
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to open device: {}", e)
            ))?;
            
        let size = file.seek(SeekFrom::End(0))
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to get device size: {}", e)
            ))?;
            
        Ok(size)
    }
}

pub fn configure_raspberry_pi(device_path: &Path, enable_ssh: bool, wifi_config: Option<(&str, &str)>) -> Result<()> {
    // Find the boot partition
    let boot_partition = find_boot_partition(device_path)?;
    
    // Mount the boot partition
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("diskutil")
            .args(["mount", boot_partition.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to mount boot partition: {}", e)
            ))?;
    }
    
    // Find mount point
    let mount_point = get_mount_point(&boot_partition)?;
    
    // Enable SSH if requested
    if enable_ssh {
        let ssh_file = mount_point.join("ssh");
        File::create(&ssh_file)
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to create SSH file: {}", e)
            ))?;
        println!("✅ SSH enabled");
    }
    
    // Configure WiFi if requested
    if let Some((ssid, password)) = wifi_config {
        let wpa_config = format!(
            r#"country=US
ctrl_interface=DIR=/var/run/wpa_supplicant GROUP=netdev
update_config=1

network={{
    ssid="{}"
    psk="{}"
    key_mgmt=WPA-PSK
}}
"#, ssid, password);
        
        let wpa_file = mount_point.join("wpa_supplicant.conf");
        let mut file = File::create(&wpa_file)
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to create WiFi config: {}", e)
            ))?;
            
        file.write_all(wpa_config.as_bytes())
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to write WiFi config: {}", e)
            ))?;
            
        println!("✅ WiFi configured for SSID: {}", ssid);
    }
    
    // Unmount
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("diskutil")
            .args(["unmount", boot_partition.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to unmount boot partition: {}", e)
            ))?;
    }
    
    Ok(())
}

fn find_boot_partition(device_path: &Path) -> Result<PathBuf> {
    // On macOS, look for the first FAT partition
    #[cfg(target_os = "macos")]
    {
        let device_name = device_path.file_name()
            .ok_or_else(|| UsbBootHutError::Device("Invalid device path".to_string()))?
            .to_string_lossy();
            
        // Try diskXs1 first (common for RPi images)
        let partition1 = PathBuf::from(format!("/dev/{}s1", device_name));
        if partition1.exists() {
            return Ok(partition1);
        }
        
        // Try diskXp1 for NVMe style
        let partition1_nvme = PathBuf::from(format!("/dev/{}p1", device_name));
        if partition1_nvme.exists() {
            return Ok(partition1_nvme);
        }
    }
    
    Err(UsbBootHutError::Device("Could not find boot partition".to_string()))
}

fn get_mount_point(partition: &Path) -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("diskutil")
            .args(["info", partition.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Device(
                format!("Failed to get partition info: {}", e)
            ))?;
            
        let info = String::from_utf8_lossy(&output.stdout);
        for line in info.lines() {
            if line.trim().starts_with("Mount Point:") {
                let mount_point = line.split(':').nth(1).unwrap_or("").trim();
                if !mount_point.is_empty() && mount_point != "(not mounted)" {
                    return Ok(PathBuf::from(mount_point));
                }
            }
        }
    }
    
    Err(UsbBootHutError::Device("Partition not mounted".to_string()))
}