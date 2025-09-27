use mtas_platform::Platform;
use adb_client::{ADBDeviceExt, ADBServer, ADBTcpDevice};
use std::net::{SocketAddr, SocketAddrV4};

pub fn create_adb_server() -> ADBServer {
    let adb_info = Platform::MuMu.adb_info();

    let ipv4 = match adb_info.ip {
        std::net::IpAddr::V4(ipv4) => ipv4,
        std::net::IpAddr::V6(_) => panic!("Expected an IPv4 address, found IPv6"),
    };

    ADBServer::new(SocketAddrV4::new(ipv4, adb_info.port))
}

pub fn create_adb_device() -> ADBTcpDevice {
    let adb_info = Platform::MuMu.adb_info();

    let socket_addr = SocketAddr::new(adb_info.ip, adb_info.port);

    ADBTcpDevice::new(socket_addr).expect("Cannot find device")
}

pub fn test() {
    let mut device = create_adb_device();
    device
        .shell(&mut std::io::stdin(), Box::new(std::io::stdout()))
        .expect("Failed to run shell on device");
}
