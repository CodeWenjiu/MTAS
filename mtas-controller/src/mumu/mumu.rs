use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    sync::{Arc, mpsc::TryRecvError},
    time::Duration,
};

use crate::{Command, ControllerTrait, Return, ScreenCapture};
use anyhow::{Result, anyhow};
use ringbuf::{
    SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
    wrap::caching::Caching,
};
use tokio::{task::spawn_blocking, time::Instant};
use triple_buffer::triple_buffer;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

enum ScreenCapCommand {
    Control(bool),
}

pub struct MuMuController {
    lib: Arc<test>,
    connection: i32,
    screen_cmdtx: std::sync::mpsc::Sender<ScreenCapCommand>,
    counter: Caching<Arc<SharedRb<Heap<Duration>>>, false, true>,
}

impl ControllerTrait for MuMuController {
    fn new() -> Result<(Self, ScreenCapture)> {
        let lib = Arc::new(unsafe {
            test::new("D:\\Program\\mumu\\MuMu Player 12\\nx_device\\12.0\\shell\\sdk\\external_renderer_ipc.dll")
                            .map_err(|e| anyhow!("Failed to load DLL: {}", e))?
        });

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

        let (mut input_buffer, output_buffer) =
            triple_buffer(&vec![0u8; (width * height * 4) as usize]);

        let screen_capture = ScreenCapture {
            width: width as usize,
            height: height as usize,
            capture: output_buffer,
        };

        let (screen_cmdtx, screen_cmdrx) = std::sync::mpsc::channel::<ScreenCapCommand>();

        let lib_in = lib.clone();

        let rb = SharedRb::<Heap<Duration>>::new(10);
        let (mut prod, cons) = rb.split();

        std::thread::spawn(move || {
            tracing::info!("Thread ScreenCap Begin");

            let mut cur_width = width;
            let mut cur_height = height;

            let mut screen_on_in = false;

            loop {
                let start = Instant::now();

                match screen_cmdrx.try_recv() {
                    Ok(command) => match command {
                        ScreenCapCommand::Control(on) => screen_on_in = on,
                    },
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => break,
                }

                if !screen_on_in {
                    continue;
                }

                let result = unsafe {
                    lib_in.nemu_capture_display(
                        connection,
                        0,
                        (width * height * 4) as i32,
                        &mut cur_width,
                        &mut cur_height,
                        input_buffer.input_buffer_mut().as_mut_ptr() as *mut u8,
                    )
                };

                if cur_width != width || cur_height != height {
                    panic!("Display size changed");
                }

                if result != 0 {
                    panic!("Failed to capture display");
                }

                let _ = prod.try_push(start.elapsed());

                input_buffer.publish();
            }

            tracing::info!("Thread ScreenCap End");
        });

        Ok((
            MuMuController {
                lib,
                connection,
                screen_cmdtx,
                counter: cons,
            },
            screen_capture,
        ))
    }

    async fn execute(&mut self, command: crate::Command) -> Result<Return> {
        match command {
            Command::Tab { x, y } => self.tab(x, y).await,
            Command::Scroll { x1, y1, x2, y2, t } => self.scroll(x1, y1, x2, y2, t).await,
            Command::ControlScreenCapture { start } => self.control_screen_capture(start),
            Command::TestScreenShotDelay {} => self.test_screen_shot_delay(),
        }
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
        self.screen_cmdtx
            .send(ScreenCapCommand::Control(start))
            .expect("WTF");

        Ok(Return::Nothing)
    }

    fn test_screen_shot_delay(&mut self) -> Result<Return> {
        // Turn on screen capture so the background thread starts pushing durations.
        self.control_screen_capture(true)?;

        let mut times = Vec::with_capacity(10);

        for _ in 0..10 {
            'innerloop: loop {
                if let Some(time) = self.counter.try_pop() {
                    times.push(time);
                    break 'innerloop;
                }
            }
        }

        // Stop capturing to avoid unnecessary work after measurement.
        self.control_screen_capture(false)?;

        // Safety: we intentionally gathered exactly 10 samples above.
        times.sort();
        let trimmed = &times[1..(10 - 1)]; // Drop min & max (basic outlier trimming).
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

    use anyhow::Ok;

    use super::*;

    #[tracing::instrument]
    fn test(important_param: u64, name: &str) {
        tracing::info!("This is Just an test");
    }

    #[test]
    fn test_mumu_init() -> Result<()> {
        let _guard = mtas_logger::set_logger(std::io::stdout())?;

        test(42, "Ferris");

        let (mut controller, _screen_cap) = MuMuController::new()?;

        tracing::info!("{:?}", controller.test_screen_shot_delay());

        Ok(())
    }
}
