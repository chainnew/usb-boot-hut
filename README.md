# USB Boot Hut 🔒

A secure USB bootable drive manager with LUKS encryption support. Create multi-boot USB drives with encrypted ISO storage and automatic junk file cleanup.

## Features

- **🔐 LUKS2 Encryption**: Secure your ISOs with strong encryption
- **🚀 Multi-boot Support**: Boot multiple operating systems from one USB
- **🧹 Smart Cleanup**: Automatically remove junk files (`.DS_Store`, `Thumbs.db`, etc.)
- **📦 ISO Management**: Add, remove, and verify ISOs with checksums
- **🎨 GRUB2 Themes**: Customizable boot menu appearance
- **🔍 Device Detection**: Safely identify and format USB drives
- **⚡ Fast Operations**: Progress tracking for all long operations

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/usb-boot-hut
cd usb-boot-hut

# Build and install
cargo build --release
sudo cp target/release/usb-boot-hut /usr/local/bin/
```

### Dependencies

- Linux: `grub2`, `cryptsetup`, `sgdisk`, `mkfs.ext4`, `mkfs.fat`
- macOS: `diskutil` (built-in)
- Windows: Administrative privileges

## Quick Start

### 1. Format a USB Drive

```bash
# Basic format (no encryption)
sudo usb-boot-hut format /dev/sdb

# With LUKS encryption
sudo usb-boot-hut format /dev/sdb --encrypt

# With secure wipe first
sudo usb-boot-hut format /dev/sdb --encrypt --secure-wipe
```

### 2. Add ISOs

```bash
# Add an ISO
sudo usb-boot-hut add ubuntu-22.04-desktop-amd64.iso

# Add with checksum verification
sudo usb-boot-hut add debian-12.iso --verify <sha256-checksum>

# Add with category and tags
sudo usb-boot-hut add kali-linux.iso --category security --tags "pentest,forensics"
```

### 3. List and Manage ISOs

```bash
# List all ISOs
usb-boot-hut list /dev/sdb

# List by category
usb-boot-hut list /dev/sdb --category linux

# Remove an ISO
sudo usb-boot-hut remove "Ubuntu 22.04"

# Verify ISO integrity
usb-boot-hut verify /dev/sdb --all
```

### 4. Clean Junk Files

```bash
# Clean with default rules
sudo usb-boot-hut clean /dev/sdb

# Dry run to see what would be deleted
sudo usb-boot-hut clean /dev/sdb --dry-run

# Use custom cleanup config
sudo usb-boot-hut clean /dev/sdb --config my-cleanup.toml
```

## USB Drive Layout

```
USB Drive
├── EFI System Partition (512MB, FAT32)
│   └── EFI/BOOT/          # UEFI bootloader
├── Boot Partition (512MB, ext4)
│   └── grub/              # GRUB configuration
└── Data Partition (remaining, ext4 or LUKS-encrypted ext4)
    ├── isos/              # ISO storage
    └── .usb-boot-hut/     # Metadata and config
```

## Configuration

### Global Config
`~/.config/usb-boot-hut/config.toml`

```toml
default_timeout = 10
default_encryption = true
auto_cleanup = false
cleanup_on_add = true
verify_checksums = true
theme = "default"
log_level = "info"
```

### Cleanup Rules
Create custom cleanup rules in TOML format:

```toml
safe_mode = true
max_file_size = 104857600  # 100MB

[[rules]]
name = "Temporary files"
enabled = true
action = "Delete"
[rules.pattern]
type = "Extension"
ext = "tmp"

[[rules]]
name = "Old logs"
enabled = true
action = "Ask"
[rules.pattern]
type = "Regex"
pattern = ".*\\.log\\.[0-9]+"
```

## Security Features

- **LUKS2 Encryption**: AES-256-XTS with Argon2id key derivation
- **Secure Wipe**: Overwrite with random data before formatting
- **Passphrase Validation**: Enforces strong passphrase requirements
- **Protected Paths**: Prevents accidental deletion of system files
- **Device Validation**: Safety checks to prevent formatting system drives

## Supported ISO Types

- **Linux**: Ubuntu, Debian, Arch, Fedora, CentOS/RHEL
- **Windows**: Windows installation ISOs (with limitations)
- **Rescue**: System rescue and recovery tools
- **Security**: Kali Linux, Parrot OS, etc.
- **Custom**: Any bootable ISO with manual configuration

## Troubleshooting

### Device Not Found
- Ensure the USB device is connected
- Check device path with `lsblk` or `usb-boot-hut devices`
- Run with sudo for device access

### Encryption Issues
- Ensure `cryptsetup` is installed
- Check kernel crypto module support
- Verify LUKS2 support with `cryptsetup --version`

### Boot Issues
- Ensure Secure Boot is disabled in BIOS
- Try Legacy BIOS mode if UEFI fails
- Check ISO compatibility with loopback booting

## Contributing

Contributions are welcome! Please read our contributing guidelines and submit pull requests to our repository.

## License

This project is dual-licensed under MIT and Apache 2.0 licenses.

## Acknowledgments

- GRUB2 project for the bootloader
- cryptsetup/LUKS for encryption
- The Rust community for excellent libraries