use std::time::Duration;
use std::thread;
use console::{style, Term};

// Cool animation frames
pub const HECTIC_FRAMES: &[&str] = &[
    " ⚡ FORMATTING ⚡ ",
    " ★ FORMATTING ★ ",
    " ▲ FORMATTING ▲ ",
    " ◆ FORMATTING ◆ ",
    " ● FORMATTING ● ",
    " ▼ FORMATTING ▼ ",
];

pub const USB_SPINNER: &[&str] = &[
    " [████    ] ",
    " [█████   ] ",
    " [██████  ] ",
    " [███████ ] ",
    " [████████] ",
    " [███████ ] ",
    " [██████  ] ",
    " [█████   ] ",
];

pub const ENCRYPTION_FRAMES: &[&str] = &[
    " 🔐 ENCRYPTING [▓         ] ",
    " 🔐 ENCRYPTING [▓▓        ] ",
    " 🔐 ENCRYPTING [▓▓▓       ] ",
    " 🔐 ENCRYPTING [▓▓▓▓      ] ",
    " 🔐 ENCRYPTING [▓▓▓▓▓     ] ",
    " 🔐 ENCRYPTING [▓▓▓▓▓▓    ] ",
    " 🔐 ENCRYPTING [▓▓▓▓▓▓▓   ] ",
    " 🔐 ENCRYPTING [▓▓▓▓▓▓▓▓  ] ",
    " 🔐 ENCRYPTING [▓▓▓▓▓▓▓▓▓ ] ",
    " 🔐 ENCRYPTING [▓▓▓▓▓▓▓▓▓▓] ",
];

pub const WIPE_ANIMATION: &[&str] = &[
    " 🧹 WIPING [░░░░░░░░░░] 0%  ",
    " 🧹 WIPING [▓░░░░░░░░░] 10% ",
    " 🧹 WIPING [▓▓░░░░░░░░] 20% ",
    " 🧹 WIPING [▓▓▓░░░░░░░] 30% ",
    " 🧹 WIPING [▓▓▓▓░░░░░░] 40% ",
    " 🧹 WIPING [▓▓▓▓▓░░░░░] 50% ",
    " 🧹 WIPING [▓▓▓▓▓▓░░░░] 60% ",
    " 🧹 WIPING [▓▓▓▓▓▓▓░░░] 70% ",
    " 🧹 WIPING [▓▓▓▓▓▓▓▓░░] 80% ",
    " 🧹 WIPING [▓▓▓▓▓▓▓▓▓░] 90% ",
    " 🧹 WIPING [▓▓▓▓▓▓▓▓▓▓] 100%",
];

pub const SUCCESS_FRAMES: &[&str] = &[
    " ✓ ",
    " ✔ ",
    " ✅ ",
];

pub struct AnimationPlayer {
    term: Term,
    running: bool,
}

impl AnimationPlayer {
    pub fn new() -> Self {
        Self {
            term: Term::stdout(),
            running: false,
        }
    }
    
    pub fn play_hectic(&mut self, message: &str) {
        self.running = true;
        let mut frame_idx = 0;
        
        while self.running {
            self.term.clear_line().ok();
            print!("\r{} {}", HECTIC_FRAMES[frame_idx], style(message).cyan());
            self.term.flush().ok();
            
            frame_idx = (frame_idx + 1) % HECTIC_FRAMES.len();
            thread::sleep(Duration::from_millis(150));
        }
    }
    
    pub fn play_usb_spinner(&mut self, message: &str) {
        self.running = true;
        let mut frame_idx = 0;
        
        while self.running {
            self.term.clear_line().ok();
            print!("\r{} {}", USB_SPINNER[frame_idx], style(message).yellow());
            self.term.flush().ok();
            
            frame_idx = (frame_idx + 1) % USB_SPINNER.len();
            thread::sleep(Duration::from_millis(100));
        }
    }
    
    pub fn play_encryption(&mut self, current_progress: u8) {
        let frame_idx = (current_progress as usize * ENCRYPTION_FRAMES.len()) / 100;
        let frame_idx = frame_idx.min(ENCRYPTION_FRAMES.len() - 1);
        
        self.term.clear_line().ok();
        print!("\r{}", style(ENCRYPTION_FRAMES[frame_idx]).green());
        self.term.flush().ok();
    }
    
    pub fn play_wipe(&mut self, current_progress: u8) {
        let frame_idx = (current_progress as usize * WIPE_ANIMATION.len()) / 100;
        let frame_idx = frame_idx.min(WIPE_ANIMATION.len() - 1);
        
        self.term.clear_line().ok();
        print!("\r{}", style(WIPE_ANIMATION[frame_idx]).red());
        self.term.flush().ok();
    }
    
    pub fn show_success(&mut self, message: &str) {
        for frame in SUCCESS_FRAMES {
            self.term.clear_line().ok();
            print!("\r{} {}", style(frame).green().bold(), style(message).green());
            self.term.flush().ok();
            thread::sleep(Duration::from_millis(200));
        }
        println!(); // New line after success
    }
    
    pub fn stop(&mut self) {
        self.running = false;
        self.term.clear_line().ok();
    }
}

// ASCII art for the banner
pub const USB_BOOT_HUT_BANNER: &str = r#"
╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   ██╗   ██╗███████╗██████╗     ██████╗  ██████╗  ██████╗ ║
║   ██║   ██║██╔════╝██╔══██╗    ██╔══██╗██╔═══██╗██╔═══██╗║
║   ██║   ██║███████╗██████╔╝    ██████╔╝██║   ██║██║   ██║║
║   ██║   ██║╚════██║██╔══██╗    ██╔══██╗██║   ██║██║   ██║║
║   ╚██████╔╝███████║██████╔╝    ██████╔╝╚██████╔╝╚██████╔╝║
║    ╚═════╝ ╚══════╝╚═════╝     ╚═════╝  ╚═════╝  ╚═════╝ ║
║                                                           ║
║              ██╗  ██╗██╗   ██╗████████╗                   ║
║              ██║  ██║██║   ██║╚══██╔══╝                   ║
║              ███████║██║   ██║   ██║                      ║
║              ██╔══██║██║   ██║   ██║                      ║
║              ██║  ██║╚██████╔╝   ██║                      ║
║              ╚═╝  ╚═╝ ╚═════╝    ╚═╝                      ║
║                                                           ║
║            🔒 Secure USB Bootable Drive Manager 🔒         ║
╚═══════════════════════════════════════════════════════════╝
"#;

pub fn print_banner() {
    println!("{}", style(USB_BOOT_HUT_BANNER).cyan().bold());
}