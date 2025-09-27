use mtas_platform::Platform;
use adb_client::ADBServer;

use std::net::SocketAddrV4;
fn main() {
    let adb_info = Platform::MuMu.adb_addr();
    let mut server = ADBServer::new(SocketAddrV4::new(adb_info.ip, adb_info.port));

    match server.devices() {
        Ok(devices) => {
            // handle devices, e.g., print or process them
            println!("{:?}", devices);
        }
        Err(e) => {
            eprintln!("Failed to get devices: {}", e);
        }
    }
}
