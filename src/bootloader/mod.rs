use crate::{Result, UsbBootHutError};
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

pub struct GrubInstaller {
    device_path: PathBuf,
}

impl GrubInstaller {
    pub fn new(device_path: &Path) -> Self {
        Self {
            device_path: device_path.to_path_buf(),
        }
    }
    
    pub fn install(&self, esp_partition: &Path, boot_partition: &Path) -> Result<()> {
        // Create temporary mount points
        let temp_dir = TempDir::new()
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create temp dir: {}", e)))?;
            
        let esp_mount = temp_dir.path().join("esp");
        let boot_mount = temp_dir.path().join("boot");
        
        fs::create_dir_all(&esp_mount)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create ESP mount: {}", e)))?;
        fs::create_dir_all(&boot_mount)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create boot mount: {}", e)))?;
        
        // Mount partitions
        self.mount_partition(esp_partition, &esp_mount)?;
        let esp_mounted = true;
        
        self.mount_partition(boot_partition, &boot_mount)?;
        let boot_mounted = true;
        
        // Install GRUB
        let result = self.install_grub_files(&esp_mount, &boot_mount);
        
        // Always unmount, even if installation failed
        if boot_mounted {
            let _ = self.unmount_partition(&boot_mount);
        }
        if esp_mounted {
            let _ = self.unmount_partition(&esp_mount);
        }
        
        result?;
        
        Ok(())
    }
    
    fn install_grub_files(&self, esp_mount: &Path, boot_mount: &Path) -> Result<()> {
        // Create necessary directories
        let efi_dir = esp_mount.join("EFI");
        let boot_dir = efi_dir.join("BOOT");
        fs::create_dir_all(&boot_dir)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create EFI dirs: {}", e)))?;
            
        let grub_dir = boot_mount.join("grub");
        fs::create_dir_all(&grub_dir)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create grub dir: {}", e)))?;
        
        // Install GRUB EFI binary
        #[cfg(target_arch = "x86_64")]
        let grub_target = "x86_64-efi";
        #[cfg(target_arch = "aarch64")]
        let grub_target = "arm64-efi";
        
        let output = Command::new("grub-install")
            .args([
                "--target", grub_target,
                "--efi-directory", esp_mount.to_str().unwrap(),
                "--boot-directory", boot_mount.to_str().unwrap(),
                "--removable",
                "--recheck",
                self.device_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to run grub-install: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Bootloader(
                format!("grub-install failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        // Create initial grub.cfg
        self.create_grub_config(&grub_dir)?;
        
        // Install theme
        self.install_theme(&grub_dir)?;
        
        Ok(())
    }
    
    fn create_grub_config(&self, grub_dir: &Path) -> Result<()> {
        let config_path = grub_dir.join("grub.cfg");
        let mut config_file = File::create(&config_path)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create grub.cfg: {}", e)))?;
            
        let config = r#"# USB Boot Hut GRUB Configuration
set timeout=10
set default=0

# Enable graphical terminal
insmod all_video
insmod gfxterm
insmod png
set gfxmode=auto
terminal_output gfxterm

# Load theme
set theme=/grub/themes/usb-boot-hut/theme.txt

# Menu colors (if theme fails)
set menu_color_normal=white/black
set menu_color_highlight=black/white

# Boot entries will be dynamically added here
# Example entry:
# menuentry "Ubuntu 22.04 Live" {
#     set isofile="/isos/ubuntu-22.04-desktop-amd64.iso"
#     loopback loop $isofile
#     linux (loop)/casper/vmlinuz boot=casper iso-scan/filename=$isofile quiet splash
#     initrd (loop)/casper/initrd
# }

menuentry "System Settings" {
    insmod part_gpt
    insmod chain
    chainloader /EFI/BOOT/BOOTX64.EFI
}

menuentry "Reboot" {
    reboot
}

menuentry "Shutdown" {
    halt
}
"#;
        
        config_file.write_all(config.as_bytes())
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to write grub.cfg: {}", e)))?;
            
        Ok(())
    }
    
    fn install_theme(&self, grub_dir: &Path) -> Result<()> {
        let theme_dir = grub_dir.join("themes/usb-boot-hut");
        fs::create_dir_all(&theme_dir)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create theme dir: {}", e)))?;
            
        // Create theme.txt
        let theme_path = theme_dir.join("theme.txt");
        let mut theme_file = File::create(&theme_path)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to create theme: {}", e)))?;
            
        let theme_config = r###"# USB Boot Hut GRUB Theme
title-text: "USB Boot Hut"
title-color: "#FFFFFF"
title-font: "DejaVu Sans Regular 24"
desktop-color: "#1a1a1a"
terminal-box: "terminal_box_*.png"

+ boot_menu {
    left = 15%
    width = 70%
    top = 30%
    height = 40%
    
    item_font = "DejaVu Sans Regular 16"
    item_color = "#CCCCCC"
    selected_item_color = "#FFFFFF"
    item_height = 32
    item_padding = 8
    item_spacing = 4
    
    selected_item_pixmap_style = "select_*.png"
}

+ progress_bar {
    id = "__timeout__"
    left = 15%
    width = 70%
    top = 75%
    height = 20
    
    font = "DejaVu Sans Regular 12"
    text_color = "#FFFFFF"
    bar_style = "progress_bar_*.png"
    highlight_style = "progress_highlight_*.png"
}

+ label {
    left = 15%
    top = 85%
    width = 70%
    align = "center"
    
    id = "__help__"
    text = "Use ↑ and ↓ keys to select, Enter to boot"
    font = "DejaVu Sans Regular 14"
    color = "#AAAAAA"
}
"###;
        
        theme_file.write_all(theme_config.as_bytes())
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to write theme: {}", e)))?;
            
        Ok(())
    }
    
    fn mount_partition(&self, partition: &Path, mount_point: &Path) -> Result<()> {
        let output = Command::new("mount")
            .args([partition.to_str().unwrap(), mount_point.to_str().unwrap()])
            .output()
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to mount: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Bootloader(
                format!("Mount failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
    
    fn unmount_partition(&self, mount_point: &Path) -> Result<()> {
        let output = Command::new("umount")
            .arg(mount_point.to_str().unwrap())
            .output()
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to unmount: {}", e)))?;
            
        if !output.status.success() {
            return Err(UsbBootHutError::Bootloader(
                format!("Unmount failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        
        Ok(())
    }
}

pub struct GrubConfigManager {
    config_path: PathBuf,
}

impl GrubConfigManager {
    pub fn new(boot_mount: &Path) -> Self {
        Self {
            config_path: boot_mount.join("grub/grub.cfg"),
        }
    }
    
    pub fn add_iso_entry(&self, iso_name: &str, iso_path: &str, boot_params: &BootParams) -> Result<()> {
        let mut config = fs::read_to_string(&self.config_path)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to read grub.cfg: {}", e)))?;
            
        // Check if entry already exists
        if config.contains(&format!("menuentry \"{}\"", iso_name)) {
            return Ok(()); // Already exists
        }
        
        // Generate menu entry based on ISO type
        let entry = match boot_params {
            BootParams::Ubuntu { version: _ } => {
                format!(r#"
menuentry "{}" {{
    set isofile="{}"
    loopback loop $isofile
    linux (loop)/casper/vmlinuz boot=casper iso-scan/filename=$isofile quiet splash
    initrd (loop)/casper/initrd
}}
"#, iso_name, iso_path)
            },
            BootParams::Debian { version: _ } => {
                format!(r#"
menuentry "{}" {{
    set isofile="{}"
    loopback loop $isofile
    linux (loop)/live/vmlinuz boot=live findiso=$isofile quiet splash
    initrd (loop)/live/initrd.img
}}
"#, iso_name, iso_path)
            },
            BootParams::Arch => {
                format!(r#"
menuentry "{}" {{
    set isofile="{}"
    loopback loop $isofile
    linux (loop)/arch/boot/x86_64/vmlinuz-linux img_dev=/dev/disk/by-label/USB_DATA img_loop=$isofile
    initrd (loop)/arch/boot/x86_64/initramfs-linux.img
}}
"#, iso_name, iso_path)
            },
            BootParams::Windows { version: _ } => {
                // Windows requires chainloading
                format!(r#"
menuentry "{}" {{
    # Windows ISOs require special handling
    # This is a placeholder - actual implementation would use wimboot
    echo "Windows direct boot not yet implemented"
    echo "Please use a Windows-to-Go installation instead"
    sleep 5
}}
"#, iso_name)
            },
            BootParams::Custom { kernel, initrd, params } => {
                format!(r#"
menuentry "{}" {{
    set isofile="{}"
    loopback loop $isofile
    linux (loop){} {}
    initrd (loop){}
}}
"#, iso_name, iso_path, kernel, params, initrd)
            },
        };
        
        // Insert before the System Settings menu entry
        let insert_pos = config.find("menuentry \"System Settings\"")
            .ok_or_else(|| UsbBootHutError::Bootloader("Invalid grub.cfg format".to_string()))?;
            
        config.insert_str(insert_pos, &entry);
        
        // Write back
        fs::write(&self.config_path, config)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to write grub.cfg: {}", e)))?;
            
        Ok(())
    }
    
    pub fn remove_iso_entry(&self, iso_name: &str) -> Result<()> {
        let config = fs::read_to_string(&self.config_path)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to read grub.cfg: {}", e)))?;
            
        let mut new_config = String::new();
        let mut skip = false;
        let entry_marker = format!("menuentry \"{}\"", iso_name);
        
        for line in config.lines() {
            if line.contains(&entry_marker) {
                skip = true;
                continue;
            }
            
            if skip && line.trim() == "}" {
                skip = false;
                continue;
            }
            
            if !skip {
                new_config.push_str(line);
                new_config.push('\n');
            }
        }
        
        fs::write(&self.config_path, new_config)
            .map_err(|e| UsbBootHutError::Bootloader(format!("Failed to write grub.cfg: {}", e)))?;
            
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum BootParams {
    Ubuntu { version: String },
    Debian { version: String },
    Arch,
    Windows { version: String },
    Custom { kernel: String, initrd: String, params: String },
}