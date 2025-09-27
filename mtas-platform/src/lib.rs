use std::{fmt::Debug, net::Ipv4Addr};

pub enum Platform {
    MuMu,
}

pub struct PlatformADBInfo {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl Debug for PlatformADBInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

impl Platform {
    pub fn adb_addr(&self) -> PlatformADBInfo {
        match self {
            Platform::MuMu => PlatformADBInfo {
                ip: Ipv4Addr::new(127, 0, 0, 1),
                port: 16384,
            },


        }
    }
}