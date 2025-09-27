use std::{fmt::Debug, net::Ipv4Addr};

pub enum Platform {
    MuMu,
}

pub struct PlatformConnectInfo {
    pub adb_ip: Ipv4Addr,
    pub adb_port: u16,
}

impl Debug for PlatformConnectInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.adb_ip, self.adb_port)
    }
}

impl Platform {
    pub fn adb_addr(&self) -> PlatformConnectInfo {
        match self {
            Platform::MuMu => PlatformConnectInfo {
                adb_ip: Ipv4Addr::new(127, 0, 0, 1),
                adb_port: 16384,
            },


        }
    }
}