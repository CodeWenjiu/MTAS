use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    sync::{
        Arc,
        mpsc::{SendError, Sender, TryRecvError},
    },
    time::{Duration, Instant},
};

use crate::{Command, ControllerTrait, Return, ScreenCapture};
use ringbuf::{
    SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
    wrap::caching::Caching,
};
use thiserror::Error;
use triple_buffer::triple_buffer;

use tracing::*;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub enum ScreenCapCommand {
    CaptureEnabled(bool),
    CaptureTimingEnabled(bool),
}

pub struct MuMuController {
    screen_cmdtx: Sender<ScreenCapCommand>,
    cons: Caching<Arc<SharedRb<Heap<Duration>>>, false, true>,
    lib: Arc<test>,
    connection: i32,
}

#[derive(Error, Debug)]
pub enum MuMuError {
    #[error("MuMu Player Not Found：{0}")]
    PathNotFound(#[from] libloading::Error),

    #[error("Screen Capture Command Failed to Send：{0}")]
    ScreenCap(#[from] SendError<ScreenCapCommand>),

    #[error("Nemu Connect Failed: {0}")]
    NemuConnect(i32),

    #[error("Nemu Get DisplayId Failed: {0}")]
    NemuGetDisplayId(i32),

    #[error("Nemu Capture Display Failed: {0}")]
    NemuCaptureDisplay(i32),

    #[error("Nemu Input Text Failed: {0}")]
    NemuInputText(i32),

    #[error("Nemu Input Event Touch Down Failed: {0}")]
    NemuInputEventTouchDown(i32),

    #[error("Nemu Input Event Touch Up Failed: {0}")]
    NemuInputEventTouchUp(i32),

    #[error("Nemu Input Event Key Down Failed: {0}")]
    NemuInputEventKeyDown(i32),

    #[error("Nemu Input Event Key Up Failed: {0}")]
    NemuInputEventKeyUp(i32),

    #[error("Nemu Input Event Finger Touch Down Failed: {0}")]
    NemuInputEventFingerTouchDown(i32),

    #[error("Nemu Input Event Finger Touch Up Failed: {0}")]
    NemuInputEventFingerTouchUp(i32),
}

impl ControllerTrait for MuMuController {
    type Error = MuMuError;

    fn new() -> Result<(Self, ScreenCapture), MuMuError> {
        let lib = Arc::new(unsafe {
            test::new(
                "D:\\Program\\mumu\\MuMu Player 12\\nx_device\\12.0\\shell\\sdk\\external_renderer_ipc.dll",
            )?
        });

        let install_path = "D:\\Program\\mumu\\MuMu Player 12";
        let os_str = OsStr::new(install_path);

        let wide_chars: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0u16)).collect();

        let connection = unsafe { lib.nemu_connect(wide_chars.as_ptr(), 0) };

        if connection <= 0 {
            return Err(MuMuError::NemuConnect(connection));
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
            return Err(MuMuError::NemuCaptureDisplay(result));
        }

        let (mut input_buffer, output_buffer) =
            triple_buffer(&vec![0u8; (width * height * 4) as usize]);

        let screen_capture = ScreenCapture {
            width: width as usize,
            height: height as usize,
            capture: output_buffer,
        };

        let (screen_cmdtx, screen_cmdrx) = std::sync::mpsc::channel::<ScreenCapCommand>();

        let rb = SharedRb::<Heap<Duration>>::new(10);
        let (mut prod, cons) = rb.split();

        let lib_in = lib.clone();

        std::thread::spawn(move || {
            info!("Thread ScreenCap Begin");

            let mut cur_width = width;
            let mut cur_height = height;

            let mut screen_on_in = false;
            let mut capture_timing_enabled = false;

            loop {
                let start = Instant::now();

                match screen_cmdrx.try_recv() {
                    Ok(command) => match command {
                        ScreenCapCommand::CaptureEnabled(on) => screen_on_in = on,
                        ScreenCapCommand::CaptureTimingEnabled(on) => capture_timing_enabled = on,
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

                if capture_timing_enabled {
                    let _ = prod.try_push(start.elapsed());
                }

                input_buffer.publish();
            }

            info!("Thread ScreenCap End");
        });

        Ok((
            MuMuController {
                screen_cmdtx,
                cons,
                lib,
                connection,
            },
            screen_capture,
        ))
    }

    #[instrument(skip_all)]
    fn execute(&mut self, command: crate::Command) -> Result<Return, MuMuError> {
        match command {
            Command::Tab { x, y } => self.tab(x, y),
            Command::Scroll { x1, y1, x2, y2, t } => self.scroll(x1, y1, x2, y2, t),
            Command::ControlScreenCapture { start } => self.control_screen_capture(start),
            Command::TestScreenShotDelay {} => self.test_screen_shot_delay(),
        }
    }
}

impl MuMuController {
    pub fn tab(&self, x: i32, y: i32) -> Result<Return, MuMuError> {
        let result_down = unsafe {
            self.lib
                .nemu_input_event_touch_down(self.connection, 0, x, y)
        };
        if result_down != 0 {
            Err(MuMuError::NemuInputEventTouchDown(result_down))
        } else {
            let result_up = unsafe { self.lib.nemu_input_event_touch_up(self.connection, 0) };
            if result_up != 0 {
                Err(MuMuError::NemuInputEventTouchUp(result_up))
            } else {
                Ok(Return::Nothing)
            }
        }
    }

    pub fn scroll(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        t: Duration,
    ) -> Result<Return, MuMuError> {
        let _ = t; // Ignore the duration for now

        let res_down = unsafe {
            self.lib
                .nemu_input_event_finger_touch_down(self.connection, 0, 1, x1, y1)
        };
        if res_down != 0 {
            return Err(MuMuError::NemuInputEventFingerTouchDown(res_down));
        }

        let res_down = unsafe {
            self.lib
                .nemu_input_event_finger_touch_down(self.connection, 0, 1, x2, y2)
        };
        if res_down != 0 {
            return Err(MuMuError::NemuInputEventFingerTouchDown(res_down));
        }

        let res_up = unsafe {
            self.lib
                .nemu_input_event_finger_touch_up(self.connection, 0, 1)
        };
        if res_up != 0 {
            return Err(MuMuError::NemuInputEventFingerTouchUp(res_up));
        }

        Ok(Return::Nothing)
    }

    pub fn control_screen_capture(&self, start: bool) -> Result<Return, MuMuError> {
        self.screen_cmdtx
            .send(ScreenCapCommand::CaptureEnabled(start))?;

        Ok(Return::Nothing)
    }

    pub fn control_screen_capture_timing(&self, start: bool) -> Result<Return, MuMuError> {
        self.screen_cmdtx
            .send(ScreenCapCommand::CaptureTimingEnabled(start))?;

        Ok(Return::Nothing)
    }

    pub fn test_screen_shot_delay(&mut self) -> Result<Return, MuMuError> {
        self.control_screen_capture(true)?;
        self.control_screen_capture_timing(true)?;

        let mut times = Vec::with_capacity(10);
        for _ in 0..10 {
            'innerloop: loop {
                if let Some(time) = self.cons.try_pop() {
                    times.push(time);
                    break 'innerloop;
                }
            }
        }

        self.control_screen_capture_timing(false)?;
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

    use anyhow::Result;

    use super::*;

    #[instrument]
    fn test(important_param: u64, name: &str) {
        info!("This is Just an test");
    }

    #[test]
    fn test_mumu_init() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        test(42, "Ferris");

        let (mut controller, _screen_cap) = MuMuController::new()?;

        info!("{:?}", controller.execute(Command::TestScreenShotDelay {}));

        Ok(())
    }
}
