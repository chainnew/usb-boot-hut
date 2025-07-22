use crate::{Result, UsbBootHutError};
use secrecy::{ExposeSecret, SecretString};
use std::path::Path;
use std::process::{Command, Stdio};
use std::io::Write;
use zeroize::Zeroize;

pub struct LuksManager {
    iter_time: u32, // milliseconds for key derivation
}

impl LuksManager {
    pub fn new() -> Self {
        Self {
            iter_time: 5000, // 5 seconds
        }
    }
    
    pub fn create_encrypted_partition(&self, device: &Path, passphrase: &str) -> Result<()> {
        // Validate passphrase strength
        self.validate_passphrase(passphrase)?;
        
        // Create LUKS2 container
        let mut child = Command::new("cryptsetup")
            .args([
                "luksFormat",
                "--type", "luks2",
                "--cipher", "aes-xts-plain64",
                "--key-size", "512",
                "--hash", "sha256",
                "--pbkdf", "argon2id",
                "--iter-time", &self.iter_time.to_string(),
                "--use-random",
                "--verify-passphrase",
                device.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| UsbBootHutError::Encryption(format!("Failed to run cryptsetup: {}", e)))?;
            
        // Write passphrase twice (for verification)
        if let Some(mut stdin) = child.stdin.take() {
            // Create a mutable copy for zeroization
            let mut pass_bytes = format!("{}\n{}\n", passphrase, passphrase).into_bytes();
            stdin.write_all(&pass_bytes)
                .map_err(|e| UsbBootHutError::Encryption(format!("Failed to write passphrase: {}", e)))?;
            pass_bytes.zeroize();
        }
        
        let output = child.wait_with_output()
            .map_err(|e| UsbBootHutError::Encryption(format!("cryptsetup failed: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Encryption(
                format!("Failed to create LUKS container: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    pub fn open_encrypted_partition(&self, device: &Path, passphrase: &str, name: &str) -> Result<()> {
        let mut child = Command::new("cryptsetup")
            .args([
                "luksOpen",
                device.to_str().unwrap(),
                name,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| UsbBootHutError::Encryption(format!("Failed to run cryptsetup: {}", e)))?;
            
        if let Some(mut stdin) = child.stdin.take() {
            let mut pass_bytes = format!("{}\n", passphrase).into_bytes();
            stdin.write_all(&pass_bytes)
                .map_err(|e| UsbBootHutError::Encryption(format!("Failed to write passphrase: {}", e)))?;
            pass_bytes.zeroize();
        }
        
        let output = child.wait_with_output()
            .map_err(|e| UsbBootHutError::Encryption(format!("cryptsetup failed: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Encryption(
                format!("Failed to open LUKS container: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    pub fn close_encrypted_partition(&self, name: &str) -> Result<()> {
        let output = Command::new("cryptsetup")
            .args(["luksClose", name])
            .output()
            .map_err(|e| UsbBootHutError::Encryption(format!("Failed to run cryptsetup: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Encryption(
                format!("Failed to close LUKS container: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    pub fn add_key_slot(&self, device: &Path, current_pass: &str, new_pass: &str) -> Result<()> {
        self.validate_passphrase(new_pass)?;
        
        let mut child = Command::new("cryptsetup")
            .args([
                "luksAddKey",
                device.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| UsbBootHutError::Encryption(format!("Failed to run cryptsetup: {}", e)))?;
            
        if let Some(mut stdin) = child.stdin.take() {
            let mut pass_bytes = format!("{}\n{}\n{}\n", current_pass, new_pass, new_pass).into_bytes();
            stdin.write_all(&pass_bytes)
                .map_err(|e| UsbBootHutError::Encryption(format!("Failed to write passphrases: {}", e)))?;
            pass_bytes.zeroize();
        }
        
        let output = child.wait_with_output()
            .map_err(|e| UsbBootHutError::Encryption(format!("cryptsetup failed: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Encryption(
                format!("Failed to add key: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    pub fn verify_luks_device(&self, device: &Path) -> Result<bool> {
        let output = Command::new("cryptsetup")
            .args(["isLuks", device.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Encryption(format!("Failed to verify LUKS: {}", e)))?;
            
        Ok(output.status.success())
    }
    
    pub fn get_luks_info(&self, device: &Path) -> Result<LuksInfo> {
        let output = Command::new("cryptsetup")
            .args(["luksDump", device.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Encryption(format!("Failed to get LUKS info: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Encryption(
                "Failed to get LUKS information".to_string()
            ));
        }
        
        // Parse luksDump output
        let dump = String::from_utf8_lossy(&output.stdout);
        let mut info = LuksInfo::default();
        
        for line in dump.lines() {
            if line.contains("Version:") {
                info.version = line.split_whitespace().last().unwrap_or("").to_string();
            } else if line.contains("Cipher:") {
                info.cipher = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.contains("PBKDF:") {
                info.pbkdf = line.split_whitespace().last().unwrap_or("").to_string();
            }
        }
        
        Ok(info)
    }
    
    fn validate_passphrase(&self, passphrase: &str) -> Result<()> {
        if passphrase.len() < 12 {
            return Err(UsbBootHutError::Encryption(
                "Passphrase must be at least 12 characters long".to_string()
            ));
        }
        
        // Check for basic complexity
        let has_upper = passphrase.chars().any(|c| c.is_uppercase());
        let has_lower = passphrase.chars().any(|c| c.is_lowercase());
        let has_digit = passphrase.chars().any(|c| c.is_numeric());
        let has_special = passphrase.chars().any(|c| !c.is_alphanumeric());
        
        if !(has_upper && has_lower && (has_digit || has_special)) {
            return Err(UsbBootHutError::Encryption(
                "Passphrase must contain uppercase, lowercase, and either numbers or special characters".to_string()
            ));
        }
        
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct LuksInfo {
    pub version: String,
    pub cipher: String,
    pub pbkdf: String,
}

// Secure passphrase handling wrapper
pub struct SecurePassphrase {
    inner: SecretString,
}

impl SecurePassphrase {
    pub fn new(passphrase: String) -> Self {
        Self {
            inner: SecretString::new(passphrase),
        }
    }
    
    pub fn expose(&self) -> &str {
        self.inner.expose_secret()
    }
}

impl Drop for SecurePassphrase {
    fn drop(&mut self) {
        // SecretString already handles zeroization
    }
}