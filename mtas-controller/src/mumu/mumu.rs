use std::{ffi::OsStr, mem::MaybeUninit, os::windows::ffi::OsStrExt, sync::Arc, time::Duration};

use crate::{Command, ControllerTrait, Return, ScreenCapture};
use anyhow::{Result, anyhow};
use tokio::{task::spawn_blocking, time::Instant};
use triple_buffer::triple_buffer;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

struct TakeWrapper<T> {
    value: MaybeUninit<T>,
}

impl<T> TakeWrapper<T> {
    fn new(value: T) -> Self {
        Self {
            value: MaybeUninit::new(value),
        }
    }

    /// Takes the value out, leaving the wrapper in an uninitialized state
    fn take(&mut self) -> T {
        unsafe { self.value.assume_init_read() }
    }

    /// Restores a value back into the wrapper
    fn restore(&mut self, value: T) {
        self.value = MaybeUninit::new(value);
    }
}

pub struct MuMuController {
    lib: Arc<test>,
    connection: i32,
    width: usize,
    height: usize,
    screen_on: bool,
    screen_cap: TakeWrapper<triple_buffer::Input<Vec<u8>>>,
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

        let (input_buffer, output_buffer) =
            triple_buffer(&vec![0u8; (width * height * 4) as usize]);

        let screen_capture = ScreenCapture {
            width: width as usize,
            height: height as usize,
            capture: output_buffer,
        };

        Ok((
            MuMuController {
                lib: Arc::new(lib),
                connection,
                width: width as usize,
                height: height as usize,
                screen_on: false,
                screen_cap: TakeWrapper::new(input_buffer),
            },
            screen_capture,
        ))
    }

    async fn execute(&mut self, command: crate::Command) -> Result<Return> {
        match command {
            Command::Tab { x, y } => self.tab(x, y).await,
            Command::Scroll { x1, y1, x2, y2, t } => self.scroll(x1, y1, x2, y2, t).await,
            Command::ControlScreenCapture { start } => self.control_screen_capture(start),
            Command::TestScreenShotDelay { iterations } => {
                self.test_screen_shot_delay(iterations).await
            }
        }
    }

    async fn capture_screen(&mut self) -> Result<()> {
        if !self.screen_on {
            return Ok(());
        }

        let screen_input = self.screen_cap.take();
        let width = self.width as i32;
        let height = self.height as i32;
        let connection = self.connection;
        let lib = self.lib.clone();

        let screen_input = spawn_blocking(move || -> Result<triple_buffer::Input<Vec<u8>>> {
            let mut cur_width = width;
            let mut cur_height = height;
            let mut screen_input = screen_input;

            let result = unsafe {
                lib.nemu_capture_display(
                    connection,
                    0,
                    (width * height * 4) as i32,
                    &mut cur_width,
                    &mut cur_height,
                    screen_input.input_buffer_mut().as_mut_ptr() as *mut u8,
                )
            };

            if cur_width != width || cur_height != height {
                panic!("Display size changed");
            }

            if result != 0 {
                return Err(anyhow!("Failed to capture display"));
            }

            screen_input.publish();

            Ok(screen_input)
        })
        .await??;

        self.screen_cap.restore(screen_input);

        Ok(())
    }
}

impl MuMuController {
    async fn tab(&mut self, x: i32, y: i32) -> Result<Return> {
        let connection = self.connection;
        let lib = self.lib.clone();

        spawn_blocking(move || {
            let result_down = unsafe { lib.nemu_input_event_touch_down(connection, 0, x, y) };
            if result_down != 0 {
                Err(anyhow!("Failed to touch down at ({}, {})", x, y))
            } else {
                let result_up = unsafe { lib.nemu_input_event_touch_up(connection, 0) };
                if result_up != 0 {
                    Err(anyhow!("Failed to touch up"))
                } else {
                    Ok(Return::Nothing)
                }
            }
        })
        .await?
    }

    async fn scroll(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, t: Duration) -> Result<Return> {
        let _ = t; // Ignore the duration for now

        let connection = self.connection;
        let lib = self.lib.clone();

        spawn_blocking(move || {
            let res_down =
                unsafe { lib.nemu_input_event_finger_touch_down(connection, 0, 1, x1, y1) };
            if res_down != 0 {
                return Err(anyhow!("Failed to touch down at ({}, {})", x1, y1));
            }

            let res_down =
                unsafe { lib.nemu_input_event_finger_touch_down(connection, 0, 1, x2, y2) };
            if res_down != 0 {
                return Err(anyhow!("Failed to touch down at ({}, {})", x2, y2));
            }
            let res_up = unsafe { lib.nemu_input_event_finger_touch_up(connection, 0, 1) };
            if res_up != 0 {
                return Err(anyhow!("Failed to touch up"));
            }
            Ok(Return::Nothing)
        })
        .await?
    }

    fn control_screen_capture(&mut self, start: bool) -> Result<Return> {
        self.screen_on = start;

        Ok(Return::Nothing)
    }

    async fn test_screen_shot_delay(&mut self, iterations: usize) -> Result<Return> {
        let mut times = Vec::new();

        let original_screen_on = self.screen_on;
        self.screen_on = true;

        for _ in 0..iterations {
            let start = Instant::now();
            self.capture_screen().await?;
            times.push(start.elapsed());
        }

        self.screen_on = original_screen_on;

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

    #[tokio::test]
    async fn test_mumu_init() -> Result<()> {
        let (mut controller, _screen_cap) = MuMuController::new()?;

        println!(
            "{:?}",
            controller
                .execute(Command::ControlScreenCapture { start: true })
                .await
        );

        println!("{:?}", controller.test_screen_shot_delay(100).await);

        Ok(())
    }
}
