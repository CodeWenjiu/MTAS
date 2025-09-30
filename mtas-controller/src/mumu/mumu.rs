use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use crate::{Command, ControllerTrait};
use anyhow::{Result, anyhow};
use image::{ImageBuffer, Rgba};

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct MuMuController {
    lib: test,
    connection: i32,
    screen_size: (i32, i32),
    screen_cap: Vec<u8>,
}

impl ControllerTrait for MuMuController {
    fn new() -> Result<Self> {
        let lib = unsafe {
            test::new("D:\\Program\\mumu\\MuMu Player 12\\nx_device\\12.0\\shell\\sdk\\external_renderer_ipc.dll")
                        .map_err(|e| anyhow!("Failed to load DLL: {}", e))?
        };

        let install_path = "D:\\Program\\mumu\\MuMu Player 12";
        let os_str = OsStr::new(install_path);

        let wide_chars: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0u16)).collect();

        let connection = unsafe { lib.nemu_connect(wide_chars.as_ptr(), 0) };

        if connection <= 0 {
            return Err(anyhow!("Connection failed, handle: {}", connection));
        }

        let mut width: i32 = 0;
        let mut height: i32 = 0;

        let result = unsafe {
            lib.nemu_capture_display(
                connection,
                0,
                0,
                &mut width,
                &mut height,
                std::ptr::null_mut::<u8>(),
            )
        };

        if result != 0 {
            return Err(anyhow!("Failed to get display size"));
        }

        Ok(MuMuController {
            lib,
            connection,
            screen_size: (width, height),
            screen_cap: vec![0; (width * height * 4) as usize],
        })
    }

    fn execute(&mut self, command: crate::Command) -> Result<()> {
        match command {
            Command::Tab { x, y } => self.tab(x, y),
            Command::Scroll { x1, y1, x2, y2 } => self.scroll(x1, y1, x2, y2),
            Command::ScreenCap => self.capture_screen(),
        }
    }
}

impl MuMuController {
    fn tab(&mut self, x: i32, y: i32) -> Result<()> {
        let result_down = unsafe {
            self.lib
                .nemu_input_event_touch_down(self.connection, 0, x, y)
        };
        if result_down != 0 {
            return Err(anyhow!("Failed to touch down at ({}, {})", x, y));
        }

        let result_up = unsafe { self.lib.nemu_input_event_touch_up(self.connection, 0) };
        if result_up != 0 {
            return Err(anyhow!("Failed to touch up"));
        }

        Ok(())
    }

    fn scroll(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()> {
        let res_down = unsafe {
            self.lib
                .nemu_input_event_finger_touch_down(self.connection, 0, 1, x1, y1)
        };
        if res_down != 0 {
            return Err(anyhow!("Failed to touch down at ({}, {})", x1, y1));
        }

        let res_down = unsafe {
            self.lib
                .nemu_input_event_finger_touch_down(self.connection, 0, 1, x2, y2)
        };
        if res_down != 0 {
            return Err(anyhow!("Failed to touch down at ({}, {})", x2, y2));
        }
        let res_up = unsafe {
            self.lib
                .nemu_input_event_finger_touch_up(self.connection, 0, 1)
        };
        if res_up != 0 {
            return Err(anyhow!("Failed to touch up"));
        }
        Ok(())
    }

    fn capture_screen(&mut self) -> Result<()> {
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
            return Err(anyhow!("Failed to capture display"));
        }

        // Save screenshot
        self.save_screenshot("screenshot.png")?;

        Ok(())
    }

    fn save_screenshot(&self, filename: &str) -> Result<()> {
        let (width, height) = self.screen_size;
        let row_size = (width * 4) as usize;
        let mut data = self.screen_cap.clone();
        // Flip the pixel data vertically in-place
        let height_usize = height as usize;
        for i in 0..(height_usize / 2) {
            let start1 = i * row_size;
            let start2 = (height_usize - 1 - i) * row_size;
            let (left, right) = data.split_at_mut(start2);
            left[start1..start1 + row_size].swap_with_slice(&mut right[0..row_size]);
        }
        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(width as u32, height as u32, data)
                .ok_or_else(|| anyhow!("Failed to create image buffer"))?;
        img.save(filename)
            .map_err(|e| anyhow!("Failed to save image: {}", e))?;
        Ok(())
    }
}

impl Drop for MuMuController {
    fn drop(&mut self) {
        unsafe { self.lib.nemu_disconnect(self.connection) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mumu_init() -> Result<()> {
        let mut controller = MuMuController::new()?;

        // controller.scroll(1000, 500, 500, 500)?;
        controller.capture_screen()?;
        // controller.save_screenshot("screenshot.png")?;
        Ok(())
    }
}
