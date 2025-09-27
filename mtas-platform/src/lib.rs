use std::{fmt::Debug, net::{IpAddr, Ipv4Addr}};

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