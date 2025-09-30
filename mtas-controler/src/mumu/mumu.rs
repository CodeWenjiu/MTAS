use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use crate::{Command, controller_trait};
use image::{ImageBuffer, Rgba};

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct MuMuControler {
    lib: test,
    connection: i32,
    screen_size: (i32, i32),
    screen_cap: Vec<u8>,
}

impl controller_trait for MuMuControler {
    fn new() -> Self {
        let lib = unsafe {
            test::new("D:\\Program\\mumu\\MuMu Player 12\\nx_device\\12.0\\shell\\sdk\\external_renderer_ipc.dll")
                        .expect("Failed to load DLL")
        };

        let install_path = "D:\\Program\\mumu\\MuMu Player 12";
        let os_str = OsStr::new(install_path);

        let wide_chars: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0u16)).collect();

        let connection = unsafe { lib.nemu_connect(wide_chars.as_ptr(), 0) };

        if connection > 0 {
            println!("Connected successfully, handle: {}", connection);
        } else {
            panic!("Connection failed, handle: {}", connection);
        }

        let mut width: i32 = 0;
        let mut height: i32 = 0;

        let result = unsafe {
            lib.nemu_capture_display(
                connection,
                0,
                0,
                &mut width as *mut i32,
                &mut height as *mut i32,
                std::ptr::null_mut::<u8>(),
            )
        };

        if result != 0 {
            panic!("Failed to capture display");
        } else {
            println!(
                "Captured display successfully, width: {}, height: {}",
                width, height
            );
        }

        MuMuControler {
            lib,
            connection,
            screen_size: (width, height),
            screen_cap: vec![0; (width * height * 4) as usize],
        }
    }

    fn execute(&mut self, command: crate::Command) {
        match command {
            Command::Tab { x, y } => self.tab(x, y),
            Command::ScreenCap => self.screen_capture(),
        }
    }
}

impl MuMuControler {
    fn tab(&mut self, x: i32, y: i32) {
        let lib = unsafe {
            test::new("D:\\Program\\mumu\\MuMu Player 12\\nx_device\\12.0\\shell\\sdk\\external_renderer_ipc.dll")
                .expect("Failed to load DLL")
        };

        let result = unsafe { lib.nemu_input_event_touch_down(self.connection, 0, x, y) };

        if result == 0 {
            println!("Tabbed successfully");
        } else {
            println!("Failed to tab");
        }

        let result = unsafe { lib.nemu_input_event_touch_up(self.connection, 0) };

        if result == 0 {
            println!("Tabbed successfully");
        } else {
            println!("Failed to tab");
        }
    }

    fn screen_capture(&mut self) {
        let mut width = self.screen_size.0;
        let mut height = self.screen_size.1;

        let result = unsafe {
            self.lib.nemu_capture_display(
                self.connection,
                0,
                self.screen_cap.len() as i32,
                &mut width,
                &mut height,
                &mut self.screen_cap[0] as *mut u8,
            )
        };

        if result != 0 {
            panic!("Failed to capture display");
        } else {
            println!(
                "Captured display successfully, width: {}, height: {}",
                width, height
            );
            // Save screenshot
            if let Err(e) = self.save_screenshot("screenshot.png") {
                eprintln!("Failed to save screenshot: {}", e);
            }
        }
    }

    fn save_screenshot(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (width, height) = self.screen_size;
        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(width as u32, height as u32, self.screen_cap.clone())
                .ok_or("Failed to create image buffer")?;
        img.save(filename)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mumu_init() {
        let mut controller = MuMuControler::new();

        controller.screen_capture();
        controller.save_screenshot("screenshot.png").unwrap();
    }
}
