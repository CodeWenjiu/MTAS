use std::{ffi::OsStr, os::windows::ffi::OsStrExt, time::Duration};

use crate::{Command, ControllerTrait, Return};
use anyhow::{Result, anyhow};
use image::{ImageBuffer, Rgba};
use tokio::time::Instant;
use triple_buffer::triple_buffer;

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct MuMuController {
    lib: test,
    connection: i32,
    width: usize,
    height: usize,
    screen_cap: (
        triple_buffer::Input<Vec<u32>>,
        triple_buffer::Output<Vec<u32>>,
    ),
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

        let screen_cap = triple_buffer(&vec![0u32; (width * height) as usize]);

        Ok(MuMuController {
            lib,
            connection,
            width: width as usize,
            height: height as usize,
            screen_cap,
        })
    }

    fn execute(&mut self, command: crate::Command) -> Result<Return> {
        match command {
            Command::Tab { x, y } => self.tab(x, y),
            Command::Scroll { x1, y1, x2, y2, t } => self.scroll(x1, y1, x2, y2, t),
            Command::TestScreenShotDelay { iterations } => self.test_screen_shot_delay(iterations),
        }
    }

    fn capture_screen(&mut self) -> Result<()> {
        let mut width = self.width as i32;
        let mut height = self.height as i32;

        let screen_input = &mut self.screen_cap.0;

        let result = unsafe {
            self.lib.nemu_capture_display(
                self.connection,
                0,
                (width * height * 4) as i32,
                &mut width,
                &mut height,
                screen_input.input_buffer_mut().as_mut_ptr() as *mut u8,
            )
        };

        if width != self.width as i32 || height != self.height as i32 {
            panic!("Display size changed");
        }

        if result != 0 {
            return Err(anyhow!("Failed to capture display"));
        }

        screen_input.publish();

        Ok(())
    }
}

impl MuMuController {
    fn tab(&mut self, x: i32, y: i32) -> Result<Return> {
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

        Ok(Return::Nothing)
    }

    fn scroll(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, t: Duration) -> Result<Return> {
        let _ = t; // Ignore the duration for now

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

        Ok(Return::Nothing)
    }

    fn test_screen_shot_delay(&mut self, iterations: usize) -> Result<Return> {
        let mut times = Vec::new();
        for _ in 0..iterations {
            let start = Instant::now();
            self.capture_screen()?;
            let elapsed = start.elapsed();
            times.push(elapsed);
        }

        let bias = iterations / 10;

        times.sort();
        let trimmed = &times[bias..(iterations - bias)];
        let total: Duration = trimmed.iter().sum();
        let ave = total / trimmed.len() as u32;

        Ok(Return::Delay(ave))
    }

    #[allow(dead_code)]
    fn save_screenshot(&mut self, filename: &str) -> Result<()> {
        let width = self.width as u32;
        let height = self.height as u32;
        let screen_output = self.screen_cap.1.read();
        let mut flipped = Vec::with_capacity(screen_output.len());
        for y in (0..self.height).rev() {
            let start = y * self.width;
            flipped.extend_from_slice(&screen_output[start..start + self.width]);
        }
        let data: Vec<u8> = flipped
            .into_iter()
            .flat_map(|pixel| pixel.to_le_bytes())
            .collect();
        let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, data)
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
        controller.save_screenshot("screenshot.png")?;
        Ok(())
    }
}
