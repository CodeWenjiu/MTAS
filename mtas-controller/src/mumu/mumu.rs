use std::{ffi::OsStr, os::windows::ffi::OsStrExt, time::Duration};

use crate::{Command, ControllerTrait, Return, ScreenCapture};
use anyhow::{Result, anyhow};
use tokio::time::Instant;
use triple_buffer::triple_buffer;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct MuMuController {
    lib: test,
    connection: i32,
    width: usize,
    height: usize,
    screen_on: bool,
    screen_cap: triple_buffer::Input<Vec<u32>>,
}

impl ControllerTrait for MuMuController {
    fn new() -> Result<(Self, ScreenCapture)> {
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

        let (input_buffer, output_buffer) = triple_buffer(&vec![0u32; (width * height) as usize]);

        let screen_capture = ScreenCapture {
            width: width as usize,
            height: height as usize,
            capture: output_buffer,
        };

        Ok((
            MuMuController {
                lib,
                connection,
                width: width as usize,
                height: height as usize,
                screen_on: false,
                screen_cap: input_buffer,
            },
            screen_capture,
        ))
    }

    fn execute(&mut self, command: crate::Command) -> Result<Return> {
        match command {
            Command::Tab { x, y } => self.tab(x, y),
            Command::Scroll { x1, y1, x2, y2, t } => self.scroll(x1, y1, x2, y2, t),
            Command::ControlScreenCapture { start } => self.control_screen_capture(start),
            Command::TestScreenShotDelay { iterations } => self.test_screen_shot_delay(iterations),
        }
    }

    fn capture_screen(&mut self) -> Result<()> {
        if !self.screen_on {
            return Ok(());
        }

        let mut width = self.width as i32;
        let mut height = self.height as i32;

        let screen_input = &mut self.screen_cap;

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

    fn control_screen_capture(&mut self, start: bool) -> Result<Return> {
        println!("Change Once");
        self.screen_on = start;

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
        let (mut controller, _screen_cap) = MuMuController::new()?;

        // controller.scroll(1000, 500, 500, 500)?;
        controller.capture_screen()?;

        Ok(())
    }
}
