use std::{fmt::Debug, net::{IpAddr, Ipv4Addr}};
use adb_client::{ADBDeviceExt, ADBTcpDevice};
use tokio::sync::mpsc;

pub enum Platform {
    MuMu,
}


pub struct PlatformADBInfo {
    pub ip: IpAddr,
    pub port: u16,
}

impl Debug for PlatformADBInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

impl Platform {
    pub fn adb_info(&self) -> PlatformADBInfo {
        match self {
            Platform::MuMu => PlatformADBInfo {
                ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port: 16384,
            },


        }
    }
}

pub struct ADBDevice {
    device: ADBTcpDevice,
}

impl ADBDevice {
    pub fn new(platform: Platform) -> std::io::Result<Self> {
        let adb_info = platform.adb_info();
        let socket_addr = std::net::SocketAddr::new(adb_info.ip, adb_info.port);
        let device = ADBTcpDevice::new(socket_addr).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("ADBTcpDevice error: {}", e))
        })?;
        Ok(ADBDevice { device })
    }

    fn command(&mut self, command: &[&str]) {
        self.device
            .shell_command(command, &mut std::io::stdout())
            .expect("Failed to run shell on device");
    }

    pub fn into_command_handler(
        mut self,
    ) -> (tokio::task::JoinHandle<Result<(), anyhow::Error>>, mpsc::Sender<Vec<String>>) {
        let (tx, mut rx) = mpsc::channel::<Vec<String>>(32);

        (tokio::task::spawn_blocking(move || {
            println!("ADB command handler started.");

            while let Some(cmd_parts) = rx.blocking_recv() {
                println!("Received command: {:?}", cmd_parts);
                let cmd_slices: Vec<&str> = cmd_parts.iter().map(AsRef::as_ref).collect();
                self.command(&cmd_slices);
                println!("Command executed.");
            }
            println!("ADB command handler finished.");
            Ok(())
        }), tx)
    }
}
