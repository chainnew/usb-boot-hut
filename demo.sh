#!/bin/bash

echo "=== USB Boot Hut Demo Script ==="
echo "This demonstrates the various commands available"
echo

# Show version
echo "1. Checking version:"
./target/release/usb-boot-hut --version
echo

# List devices
echo "2. Listing USB devices:"
./target/release/usb-boot-hut devices
echo

# Show devices in different formats
echo "3. Devices in simple format:"
./target/release/usb-boot-hut devices --format simple
echo

# Show configuration
echo "4. Current configuration:"
./target/release/usb-boot-hut config show
echo

# Show what formatting would do (without actually doing it)
echo "5. Format command help (what it would do):"
echo "./target/release/usb-boot-hut format /dev/disk7 --encrypt"
echo "This would:"
echo "  - Validate the device is removable and large enough"
echo "  - Create GPT partition table"
echo "  - Create 3 partitions: ESP (512MB), Boot (512MB), Data (rest)"
echo "  - Encrypt the data partition with LUKS2"
echo "  - Install GRUB2 bootloader"
echo

# Show cleanup rules
echo "6. Default cleanup rules:"
./target/release/usb-boot-hut config generate-cleanup /tmp/demo-cleanup.toml
head -20 /tmp/demo-cleanup.toml
echo "..."
echo

# Show ISO management commands
echo "7. ISO management commands:"
echo "./target/release/usb-boot-hut add ubuntu-22.04.iso --verify <sha256>"
echo "./target/release/usb-boot-hut list /dev/disk7"
echo "./target/release/usb-boot-hut remove 'Ubuntu 22.04'"
echo

echo "=== End of Demo ==="