use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use crate::{Result, UsbBootHutError};
use crate::iso::IsoType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoMetadata {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub iso_type: IsoType,
    pub size: u64,
    pub checksum: String,
    pub added_date: chrono::DateTime<chrono::Utc>,
    pub last_verified: Option<chrono::DateTime<chrono::Utc>>,
    pub boot_params: Option<BootConfiguration>,
    pub category: IsoCategory,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootConfiguration {
    pub kernel: String,
    pub initrd: String,
    pub params: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IsoCategory {
    Linux,
    Windows,
    Rescue,
    Utility,
    Security,
    Custom,
}

pub struct MetadataStore {
    store_path: PathBuf,
    metadata: Vec<IsoMetadata>,
}

impl MetadataStore {
    pub fn new(data_mount: &Path) -> Result<Self> {
        let store_path = data_mount.join(".usb-boot-hut/metadata.json");
        
        // Ensure directory exists
        if let Some(parent) = store_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to create metadata dir: {}", e)))?;
        }
        
        // Load existing metadata or create new
        let metadata = if store_path.exists() {
            let content = fs::read_to_string(&store_path)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to read metadata: {}", e)))?;
            serde_json::from_str(&content)
                .map_err(|e| UsbBootHutError::Iso(format!("Failed to parse metadata: {}", e)))?
        } else {
            Vec::new()
        };
        
        Ok(Self {
            store_path,
            metadata,
        })
    }
    
    pub fn add_iso(&mut self, metadata: IsoMetadata) -> Result<()> {
        // Check for duplicates
        if self.metadata.iter().any(|m| m.id == metadata.id) {
            return Err(UsbBootHutError::Iso("ISO already exists".to_string()));
        }
        
        self.metadata.push(metadata);
        self.save()?;
        Ok(())
    }
    
    pub fn remove_iso(&mut self, iso_id: &str) -> Result<()> {
        let original_len = self.metadata.len();
        self.metadata.retain(|m| m.id != iso_id);
        
        if self.metadata.len() == original_len {
            return Err(UsbBootHutError::Iso("ISO not found".to_string()));
        }
        
        self.save()?;
        Ok(())
    }
    
    pub fn update_iso(&mut self, iso_id: &str, metadata: IsoMetadata) -> Result<()> {
        if let Some(pos) = self.metadata.iter().position(|m| m.id == iso_id) {
            self.metadata[pos] = metadata;
            self.save()?;
            Ok(())
        } else {
            Err(UsbBootHutError::Iso("ISO not found".to_string()))
        }
    }
    
    pub fn get_iso(&self, iso_id: &str) -> Option<&IsoMetadata> {
        self.metadata.iter().find(|m| m.id == iso_id)
    }
    
    pub fn list_by_category(&self, category: IsoCategory) -> Vec<&IsoMetadata> {
        self.metadata.iter()
            .filter(|m| m.category == category)
            .collect()
    }
    
    pub fn list_all(&self) -> &[IsoMetadata] {
        &self.metadata
    }
    
    pub fn search(&self, query: &str) -> Vec<&IsoMetadata> {
        let query_lower = query.to_lowercase();
        self.metadata.iter()
            .filter(|m| {
                m.display_name.to_lowercase().contains(&query_lower) ||
                m.filename.to_lowercase().contains(&query_lower) ||
                m.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
    
    fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.metadata)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to serialize metadata: {}", e)))?;
            
        fs::write(&self.store_path, json)
            .map_err(|e| UsbBootHutError::Iso(format!("Failed to write metadata: {}", e)))?;
            
        Ok(())
    }
}

impl IsoMetadata {
    pub fn new(filename: String, iso_type: IsoType, size: u64, checksum: String) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let display_name = Self::generate_display_name(&filename, &iso_type);
        let category = Self::determine_category(&iso_type);
        
        Self {
            id,
            filename,
            display_name,
            iso_type,
            size,
            checksum,
            added_date: chrono::Utc::now(),
            last_verified: None,
            boot_params: None,
            category,
            tags: Vec::new(),
        }
    }
    
    fn generate_display_name(filename: &str, iso_type: &IsoType) -> String {
        // Extract version info from filename
        let base = filename.trim_end_matches(".iso");
        
        match iso_type {
            IsoType::Ubuntu => {
                if let Some(caps) = regex::Regex::new(r"ubuntu-(\d+\.\d+)")
                    .unwrap()
                    .captures(base) 
                {
                    format!("Ubuntu {}", &caps[1])
                } else {
                    "Ubuntu".to_string()
                }
            },
            IsoType::Debian => {
                if let Some(caps) = regex::Regex::new(r"debian-(\d+)")
                    .unwrap()
                    .captures(base)
                {
                    format!("Debian {}", &caps[1])
                } else {
                    "Debian".to_string()
                }
            },
            _ => base.replace('-', " ").replace('_', " ")
                .split_whitespace()
                .map(|w| {
                    let mut chars = w.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().chain(chars).collect()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
    
    fn determine_category(iso_type: &IsoType) -> IsoCategory {
        match iso_type {
            IsoType::Windows => IsoCategory::Windows,
            IsoType::Ubuntu | IsoType::Debian | IsoType::Arch | 
            IsoType::Fedora | IsoType::RedHat => IsoCategory::Linux,
            IsoType::Unknown => IsoCategory::Custom,
        }
    }
}