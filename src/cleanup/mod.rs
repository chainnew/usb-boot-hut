use crate::{Result, UsbBootHutError};
use std::path::{Path, PathBuf};
use std::fs;
use walkdir::WalkDir;
use serde::{Deserialize, Serialize};
use regex::Regex;
use colored::*;
use dialoguer::Confirm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    pub rules: Vec<CleanupRule>,
    pub safe_mode: bool,
    pub max_file_size: Option<u64>,
    pub protected_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupRule {
    pub name: String,
    pub pattern: Pattern,
    pub action: CleanupAction,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Pattern {
    Extension { ext: String },
    ExactName { name: String },
    Prefix { prefix: String },
    Suffix { suffix: String },
    Regex { pattern: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupAction {
    Delete,
    Skip,
    Ask,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            rules: vec![
                CleanupRule {
                    name: "Temporary files".to_string(),
                    pattern: Pattern::Extension { ext: "tmp".to_string() },
                    action: CleanupAction::Delete,
                    enabled: true,
                },
                CleanupRule {
                    name: "macOS metadata".to_string(),
                    pattern: Pattern::ExactName { name: ".DS_Store".to_string() },
                    action: CleanupAction::Delete,
                    enabled: true,
                },
                CleanupRule {
                    name: "macOS resource forks".to_string(),
                    pattern: Pattern::Prefix { prefix: "._".to_string() },
                    action: CleanupAction::Delete,
                    enabled: true,
                },
                CleanupRule {
                    name: "Windows thumbnails".to_string(),
                    pattern: Pattern::ExactName { name: "Thumbs.db".to_string() },
                    action: CleanupAction::Delete,
                    enabled: true,
                },
                CleanupRule {
                    name: "Windows desktop.ini".to_string(),
                    pattern: Pattern::ExactName { name: "desktop.ini".to_string() },
                    action: CleanupAction::Delete,
                    enabled: true,
                },
                CleanupRule {
                    name: "Backup files".to_string(),
                    pattern: Pattern::Suffix { suffix: "~".to_string() },
                    action: CleanupAction::Ask,
                    enabled: true,
                },
                CleanupRule {
                    name: "Log files".to_string(),
                    pattern: Pattern::Extension { ext: "log".to_string() },
                    action: CleanupAction::Ask,
                    enabled: true,
                },
            ],
            safe_mode: true,
            max_file_size: Some(100 * 1024 * 1024), // 100MB
            protected_paths: vec![
                "/isos".to_string(),
                "/grub".to_string(),
                "/.usb-boot-hut".to_string(),
            ],
        }
    }
}

pub struct CleanupEngine {
    config: CleanupConfig,
    dry_run: bool,
}

impl CleanupEngine {
    pub fn new(config: CleanupConfig) -> Self {
        Self {
            config,
            dry_run: false,
        }
    }
    
    pub fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
    
    pub fn load_config(config_path: &Path) -> Result<CleanupConfig> {
        let content = fs::read_to_string(config_path)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to read config: {}", e)))?;
            
        toml::from_str(&content)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to parse config: {}", e)))
    }
    
    pub fn save_default_config(config_path: &Path) -> Result<()> {
        let config = CleanupConfig::default();
        let content = toml::to_string_pretty(&config)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to serialize config: {}", e)))?;
            
        fs::write(config_path, content)
            .map_err(|e| UsbBootHutError::Config(format!("Failed to write config: {}", e)))?;
            
        Ok(())
    }
    
    pub fn clean(&self, target_path: &Path) -> Result<CleanupStats> {
        let mut stats = CleanupStats::default();
        let mut files_to_delete = Vec::new();
        
        println!("{}", "🔍 Scanning for files to clean...".cyan());
        
        // Walk the directory tree
        for entry in WalkDir::new(target_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            // Skip directories
            if path.is_dir() {
                continue;
            }
            
            // Check if path is protected
            if self.is_protected(path, target_path) {
                continue;
            }
            
            // Get file metadata
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            
            // Check file size limit
            if let Some(max_size) = self.config.max_file_size {
                if metadata.len() > max_size {
                    stats.skipped_large += 1;
                    continue;
                }
            }
            
            // Check against rules
            if let Some((rule, action)) = self.match_file(path) {
                match action {
                    CleanupAction::Delete => {
                        files_to_delete.push((path.to_path_buf(), rule.name.clone(), metadata.len()));
                        stats.matched += 1;
                    },
                    CleanupAction::Ask => {
                        if self.ask_user(path, &rule.name, metadata.len())? {
                            files_to_delete.push((path.to_path_buf(), rule.name.clone(), metadata.len()));
                            stats.matched += 1;
                        } else {
                            stats.skipped_user += 1;
                        }
                    },
                    CleanupAction::Skip => {
                        stats.skipped_rule += 1;
                    }
                }
            }
            
            stats.scanned += 1;
        }
        
        // Show summary and confirm
        if files_to_delete.is_empty() {
            println!("{}", "✨ No files to clean!".green());
            return Ok(stats);
        }
        
        println!("\n{}", format!("Found {} files to clean:", files_to_delete.len()).yellow());
        
        // Group by rule
        let mut by_rule: std::collections::HashMap<String, Vec<(&PathBuf, u64)>> = std::collections::HashMap::new();
        for (path, rule, size) in &files_to_delete {
            by_rule.entry(rule.clone()).or_insert_with(Vec::new).push((path, *size));
        }
        
        for (rule, files) in by_rule {
            let total_size: u64 = files.iter().map(|(_, s)| s).sum();
            println!("  {} {}: {} files ({})",
                "•".cyan(),
                rule,
                files.len(),
                format_size(total_size)
            );
        }
        
        let total_size: u64 = files_to_delete.iter().map(|(_, _, s)| s).sum();
        println!("\n{}", format!("Total size to free: {}", format_size(total_size)).bold());
        
        // Confirm deletion
        if !self.dry_run && self.config.safe_mode {
            if !Confirm::new()
                .with_prompt("Proceed with cleanup?")
                .default(false)
                .interact()
                .unwrap_or(false)
            {
                println!("{}", "Cleanup cancelled".yellow());
                return Ok(stats);
            }
        }
        
        // Delete files
        if self.dry_run {
            println!("\n{}", "DRY RUN - No files were deleted".yellow());
        } else {
            println!("\n{}", "🗑️  Deleting files...".red());
            for (path, _, size) in files_to_delete {
                match fs::remove_file(&path) {
                    Ok(_) => {
                        stats.deleted += 1;
                        stats.bytes_freed += size;
                    },
                    Err(e) => {
                        println!("  {} Failed to delete {}: {}", 
                            "✗".red(), 
                            path.display(), 
                            e
                        );
                        stats.errors += 1;
                    }
                }
            }
        }
        
        Ok(stats)
    }
    
    fn is_protected(&self, path: &Path, base_path: &Path) -> bool {
        let rel_path = match path.strip_prefix(base_path) {
            Ok(p) => p,
            Err(_) => return false,
        };
        
        let rel_str = rel_path.to_string_lossy();
        
        self.config.protected_paths.iter().any(|protected| {
            rel_str.starts_with(protected.trim_start_matches('/'))
        })
    }
    
    fn match_file(&self, path: &Path) -> Option<(&CleanupRule, &CleanupAction)> {
        let filename = path.file_name()?.to_str()?;
        
        for rule in &self.config.rules {
            if !rule.enabled {
                continue;
            }
            
            let matches = match &rule.pattern {
                Pattern::Extension { ext } => {
                    path.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e == ext)
                        .unwrap_or(false)
                },
                Pattern::ExactName { name } => filename == name,
                Pattern::Prefix { prefix } => filename.starts_with(prefix),
                Pattern::Suffix { suffix } => filename.ends_with(suffix),
                Pattern::Regex { pattern } => {
                    Regex::new(pattern).ok()?.is_match(filename)
                },
            };
            
            if matches {
                return Some((rule, &rule.action));
            }
        }
        
        None
    }
    
    fn ask_user(&self, path: &Path, rule_name: &str, size: u64) -> Result<bool> {
        println!("\n{} {}", "?".yellow(), path.display());
        println!("  Rule: {} | Size: {}", rule_name, format_size(size));
        
        Ok(Confirm::new()
            .with_prompt("Delete this file?")
            .default(false)
            .interact()
            .unwrap_or(false))
    }
}

#[derive(Debug, Default)]
pub struct CleanupStats {
    pub scanned: u64,
    pub matched: u64,
    pub deleted: u64,
    pub skipped_user: u64,
    pub skipped_rule: u64,
    pub skipped_large: u64,
    pub errors: u64,
    pub bytes_freed: u64,
}

impl CleanupStats {
    pub fn print_summary(&self) {
        println!("\n{}", "=== Cleanup Summary ===".bold());
        println!("  Files scanned:  {}", self.scanned);
        println!("  Files matched:  {}", self.matched);
        println!("  Files deleted:  {}", self.deleted.to_string().green());
        println!("  Files skipped:  {}", 
            (self.skipped_user + self.skipped_rule + self.skipped_large)
        );
        println!("  Errors:         {}", 
            if self.errors > 0 { 
                self.errors.to_string().red() 
            } else { 
                "0".normal() 
            }
        );
        println!("  Space freed:    {}", 
            format_size(self.bytes_freed).green()
        );
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    
    if unit_idx == 0 {
        format!("{} {}", size as u64, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}