use usb_boot_hut::*;

#[test]
fn test_library_loads() {
    // Basic test to ensure the library compiles
    assert_eq!(APP_NAME, "USB Boot Hut");
    assert!(MIN_DRIVE_SIZE > 0);
}

#[test]
fn test_error_types() {
    use usb_boot_hut::UsbBootHutError;
    
    let err = UsbBootHutError::Device("Test error".to_string());
    assert!(err.to_string().contains("Test error"));
}