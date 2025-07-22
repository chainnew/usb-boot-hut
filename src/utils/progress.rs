use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::Duration;

pub struct ProgressManager {
    multi: MultiProgress,
}

impl ProgressManager {
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
        }
    }
    
    pub fn create_spinner(&self, message: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }
    
    pub fn create_progress_bar(&self, total: u64, message: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message(message.to_string());
        pb
    }
    
    pub fn create_bytes_progress(&self, total: u64, message: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("█▓▒░ ")
        );
        pb.set_message(message.to_string());
        pb
    }
    
    pub fn create_percentage_bar(&self, message: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(100));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n[{elapsed_precise}] [{bar:40.green/red}] {pos}%")
                .unwrap()
                .progress_chars("▓▓░")
        );
        pb.set_message(message.to_string());
        pb
    }
}

// Helper function for quick progress bars
pub fn with_progress<F, T>(total: u64, message: &str, mut f: F) -> T
where
    F: FnMut(&ProgressBar) -> T,
{
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("##-")
    );
    pb.set_message(message.to_string());
    
    let result = f(&pb);
    pb.finish_with_message("Done");
    result
}