use mtas_platform::Platform;
use adb_client::ADBTcpDevice;
use std::net::SocketAddr;

pub fn create_adb_device() -> ADBTcpDevice {
    let adb_info = Platform::MuMu.adb_info();

    let socket_addr = SocketAddr::new(adb_info.ip, adb_info.port);

    ADBTcpDevice::new(socket_addr).expect("Cannot find device")
}

#[test]
pub fn test_connect() {
    use adb_client::ADBDeviceExt;
    let mut device = create_adb_device();
    device
        .shell_command(&["input", "tap", "500", "500"], &mut std::io::stdout())
        .expect("Failed to run shell on device");
}