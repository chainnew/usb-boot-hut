use crate::{Result, UsbBootHutError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use dirs::config_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_timeout: u32,
    pub default_encryption: bool,
    pub auto_cleanup: bool,
    pub cleanup_on_add: bool,
    pub verify_checksums: bool,
    pub theme: String,
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_timeout: 10,
            default_encryption: true,
            auto_cleanup: false,
            cleanup_on_add: true,
            verify_checksums: true,
            theme: "default".to_string(),
            log_level: "info".to_string(),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    config: AppConfig,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_dir = config_dir()
            .ok_or_else(|| UsbBootHutError::Config("Failed to get config directory".to_string()))?
            .join("usb-boot-hut");
            
        fs::create_dir_all(&config_dir)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to create config dir: {}", e)))?;
            
        let config_path = config_dir.join("config.toml");
        
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            let default_config = AppConfig::default();
            Self::save_config(&config_path, &default_config)?;
            default_config
        };
        
        Ok(Self {
            config_path,
            config,
        })
    }
    
    pub fn from_file(config_path: &Path) -> Result<Self> {
        let config = Self::load_config(config_path)?;
        Ok(Self {
            config_path: config_path.to_path_buf(),
            config,
        })
    }
    
    fn load_config(path: &Path) -> Result<AppConfig> {
        let content = fs::read_to_string(path)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to read config: {}", e)))?;
            
        toml::from_str(&content)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to parse config: {}", e)))
    }
    
    fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to serialize config: {}", e)))?;
            
        fs::write(path, content)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to write config: {}", e)))?;
            
        Ok(())
    }
    
    pub fn get(&self) -> &AppConfig {
        &self.config
    }
    
    pub fn get_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }
    
    pub fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }
    
    pub fn reset_to_defaults(&mut self) -> Result<()> {
        self.config = AppConfig::default();
        self.save()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub device_id: String,
    pub device_name: String,
    pub created_date: chrono::DateTime<chrono::Utc>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub encryption_enabled: bool,
    pub boot_timeout: u32,
    pub default_entry: Option<String>,
    pub theme: String,
}

impl DeviceConfig {
    pub fn new(device_id: String, device_name: String, encryption_enabled: bool) -> Self {
        let now = chrono::Utc::now();
        Self {
            device_id,
            device_name,
            created_date: now,
            last_updated: now,
            encryption_enabled,
            boot_timeout: 10,
            default_entry: None,
            theme: "default".to_string(),
        }
    }
    
    pub fn load_from_device(mount_path: &Path) -> Result<Option<Self>> {
        let config_path = mount_path.join(".usb-boot-hut/device.toml");
        
        if !config_path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(&config_path)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to read device config: {}", e)))?;
            
        let config = toml::from_str(&content)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to parse device config: {}", e)))?;
            
        Ok(Some(config))
    }
    
    pub fn save_to_device(&mut self, mount_path: &Path) -> Result<()> {
        self.last_updated = chrono::Utc::now();
        
        let config_dir = mount_path.join(".usb-boot-hut");
        fs::create_dir_all(&config_dir)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to create config dir: {}", e)))?;
            
        let config_path = config_dir.join("device.toml");
        let content = toml::to_string_pretty(self)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to serialize device config: {}", e)))?;
            
        fs::write(&config_path, content)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to write device config: {}", e)))?;
            
        Ok(())
    }
}