use crate::{Result, UsbBootHutError};
use crate::iso::{IsoValidator, IsoInfo, IsoMetadata, MetadataStore, IsoCategory};
use crate::bootloader::{GrubConfigManager, BootParams};
use crate::utils::with_progress;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Read, Write};
use indicatif::ProgressBar;

pub struct IsoManager {
    data_mount: PathBuf,
    boot_mount: PathBuf,
    metadata_store: MetadataStore,
}

impl IsoManager {
    pub fn new(data_mount: &Path, boot_mount: &Path) -> Result<Self> {
        let metadata_store = MetadataStore::new(data_mount)?;
        
        // Ensure ISO directory exists
        let iso_dir = data_mount.join("isos");
        fs::create_dir_all(&iso_dir)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to create ISO dir: {}", e)))?;
            
        Ok(Self {
            data_mount: data_mount.to_path_buf(),
            boot_mount: boot_mount.to_path_buf(),
            metadata_store,
        })
    }
    
    pub fn add_iso(&mut self, iso_path: &Path, verify_checksum: Option<&str>) -> Result<()> {
        println!("🔍 Validating ISO...");
        
        // Validate ISO
        let iso_info = IsoValidator::validate_iso(iso_path)?;
        if !iso_info.bootable {
            println!("⚠️  Warning: ISO may not be bootable");
        }
        
        // Calculate checksum
        println!("🔐 Calculating checksum...");
        let checksum = with_progress(iso_info.size, "Calculating SHA256", |pb| {
            Self::calculate_checksum_with_progress(iso_path, pb)
        })?;
        
        // Verify checksum if provided
        if let Some(expected) = verify_checksum {
            if !checksum.eq_ignore_ascii_case(expected) {
                return Err(UsbBootHutError::Iso(
                    "Checksum verification failed".to_string()
                ));
            }
            println!("✅ Checksum verified");
        }
        
        // Generate destination path
        let filename = iso_path.file_name()
            .ok_or_else(|| UsbBootHutError::Iso("Invalid ISO filename".to_string()))?
            .to_string_lossy()
            .to_string();
            
        let dest_path = self.data_mount.join("isos").join(&filename);
        
        // Check available space
        let available_space = self.get_available_space()?;
        if iso_info.size > available_space {
            return Err(UsbBootHutError::Iso(
                format!("Not enough space. Need {} bytes, have {} bytes", 
                    iso_info.size, available_space)
            ));
        }
        
        // Copy ISO with progress
        println!("📦 Copying ISO to USB drive...");
        with_progress(iso_info.size, "Copying ISO", |pb| {
            Self::copy_with_progress(iso_path, &dest_path, pb)
        })?;
        
        // Create metadata
        let metadata = IsoMetadata::new(
            filename.clone(),
            iso_info.iso_type.clone(),
            iso_info.size,
            checksum,
        );
        
        // Generate boot parameters
        let boot_params = self.generate_boot_params(&iso_info);
        
        // Update GRUB config
        let grub_mgr = GrubConfigManager::new(&self.boot_mount);
        let iso_rel_path = format!("/isos/{}", filename);
        grub_mgr.add_iso_entry(&metadata.display_name, &iso_rel_path, &boot_params)?;
        
        // Save metadata
        self.metadata_store.add_iso(metadata)?;
        
        println!("✅ ISO added successfully: {}", filename);
        Ok(())
    }
    
    pub fn remove_iso(&mut self, iso_id: &str) -> Result<()> {
        // Get metadata
        let metadata = self.metadata_store.get_iso(iso_id)
            .ok_or_else(|| UsbBootHutError::Iso("ISO not found".to_string()))?
            .clone();
            
        // Remove from GRUB config
        let grub_mgr = GrubConfigManager::new(&self.boot_mount);
        grub_mgr.remove_iso_entry(&metadata.display_name)?;
        
        // Delete ISO file
        let iso_path = self.data_mount.join("isos").join(&metadata.filename);
        if iso_path.exists() {
            fs::remove_file(&iso_path)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to delete ISO: {}", e)))?;
        }
        
        // Remove metadata
        self.metadata_store.remove_iso(iso_id)?;
        
        println!("✅ ISO removed: {}", metadata.display_name);
        Ok(())
    }
    
    pub fn list_isos(&self, category: Option<IsoCategory>) -> Vec<&IsoMetadata> {
        if let Some(cat) = category {
            self.metadata_store.list_by_category(cat)
        } else {
            self.metadata_store.list_all().iter().collect()
        }
    }
    
    pub fn verify_iso(&self, iso_id: &str) -> Result<bool> {
        let metadata = self.metadata_store.get_iso(iso_id)
            .ok_or_else(|| UsbBootHutError::Iso("ISO not found".to_string()))?;
            
        let iso_path = self.data_mount.join("isos").join(&metadata.filename);
        
        println!("🔍 Verifying ISO: {}", metadata.display_name);
        let current_checksum = IsoValidator::calculate_checksum(&iso_path)?;
        
        let valid = current_checksum == metadata.checksum;
        if valid {
            println!("✅ ISO integrity verified");
        } else {
            println!("❌ ISO integrity check failed!");
        }
        
        Ok(valid)
    }
    
    pub fn verify_all(&mut self) -> Result<()> {
        let iso_ids: Vec<String> = self.metadata_store.list_all()
            .iter()
            .map(|m| m.id.clone())
            .collect();
            
        let mut failed = Vec::new();
        
        for iso_id in iso_ids {
            match self.verify_iso(&iso_id) {
                Ok(true) => {},
                Ok(false) => failed.push(iso_id),
                Err(e) => {
                    println!("⚠️  Error verifying ISO {}: {}", iso_id, e);
                    failed.push(iso_id);
                }
            }
        }
        
        if failed.is_empty() {
            println!("\n✅ All ISOs verified successfully");
        } else {
            println!("\n❌ {} ISO(s) failed verification", failed.len());
        }
        
        Ok(())
    }
    
    fn calculate_checksum_with_progress(iso_path: &Path, pb: &ProgressBar) -> Result<String> {
        use sha2::{Sha256, Digest};
        
        let mut file = File::open(iso_path)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to open ISO: {}", e)))?;
            
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 4 * 1024 * 1024]; // 4MB chunks
        let mut total_read = 0u64;
        
        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to read: {}", e)))?;
                
            if bytes_read == 0 {
                break;
            }
            
            hasher.update(&buffer[..bytes_read]);
            total_read += bytes_read as u64;
            pb.set_position(total_read);
        }
        
        Ok(hex::encode(hasher.finalize()))
    }
    
    fn copy_with_progress(src: &Path, dest: &Path, pb: &ProgressBar) -> Result<()> {
        let mut src_file = File::open(src)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to open source: {}", e)))?;
        let mut dest_file = File::create(dest)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to create dest: {}", e)))?;
            
        let mut buffer = vec![0u8; 4 * 1024 * 1024]; // 4MB chunks
        let mut total_written = 0u64;
        
        loop {
            let bytes_read = src_file.read(&mut buffer)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to read: {}", e)))?;
                
            if bytes_read == 0 {
                break;
            }
            
            dest_file.write_all(&buffer[..bytes_read])
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to write: {}", e)))?;
                
            total_written += bytes_read as u64;
            pb.set_position(total_written);
        }
        
        dest_file.sync_all()
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to sync: {}", e)))?;
            
        Ok(())
    }
    
    fn get_available_space(&self) -> Result<u64> {
        #[cfg(target_os = "linux")]
        {
            use nix::sys::statvfs::statvfs;
            
            let stat = statvfs(&self.data_mount)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to get space: {}", e)))?;
                
            Ok(stat.blocks_available() * stat.block_size())
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Placeholder for other platforms
            Ok(0)
        }
    }
    
    fn generate_boot_params(&self, iso_info: &IsoInfo) -> BootParams {
        use crate::iso::IsoType;
        
        match iso_info.iso_type {
            IsoType::Ubuntu => BootParams::Ubuntu {
                version: iso_info.volume_id.clone()
            },
            IsoType::Debian => BootParams::Debian {
                version: iso_info.volume_id.clone()
            },
            IsoType::Arch => BootParams::Arch,
            IsoType::Windows => BootParams::Windows {
                version: iso_info.volume_id.clone()
            },
            _ => BootParams::Custom {
                kernel: "/vmlinuz".to_string(),
                initrd: "/initrd.img".to_string(),
                params: "quiet splash".to_string(),
            }
        }
    }
}