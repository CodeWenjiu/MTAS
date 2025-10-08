use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    sync::{Arc, mpsc::TryRecvError},
    time::{Duration, Instant},
};

use crate::{Command, ControllerTrait, Return, ScreenCapture};
use anyhow::{Result, anyhow};
use ringbuf::{
    SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
};
use triple_buffer::triple_buffer;

use tracing::*;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

enum ScreenCapCommand {
    CaptureEnabled(bool),
    CaptureTimingEnabled(bool),
}

pub struct MuMuController {
    execute_cmdtx: std::sync::mpsc::Sender<crate::Command>,
    result_rx: std::sync::mpsc::Receiver<Result<Return>>,
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

        let rb = SharedRb::<Heap<Duration>>::new(10);
        let (mut prod, mut cons) = rb.split();

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

        let (execute_cmdtx, execute_cmdrx) = std::sync::mpsc::channel::<crate::Command>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<Result<Return>>();

        std::thread::spawn(move || {
            info!("Thread Execute Begin");

            let tab = |x: i32, y: i32| -> Result<Return> {
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
            };

            let scroll = |x1: i32, y1: i32, x2: i32, y2: i32, t: Duration| -> Result<Return> {
                let _ = t; // Ignore the duration for now

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
            };

            let control_screen_capture = |start: bool| -> Result<Return> {
                screen_cmdtx
                    .send(ScreenCapCommand::CaptureEnabled(start))
                    .expect("WTF");

                Ok(Return::Nothing)
            };

            let control_screen_capture_timing = |start: bool| -> Result<Return> {
                screen_cmdtx
                    .send(ScreenCapCommand::CaptureTimingEnabled(start))
                    .expect("WTF");

                Ok(Return::Nothing)
            };

            let mut test_screen_shot_delay = || -> Result<Return> {
                control_screen_capture(true)?;
                control_screen_capture_timing(true)?;

                let mut times = Vec::with_capacity(10);

                for _ in 0..10 {
                    'innerloop: loop {
                        if let Some(time) = cons.try_pop() {
                            times.push(time);
                            break 'innerloop;
                        }
                    }
                }

                control_screen_capture_timing(false)?;

                times.sort();
                let trimmed = &times[1..(10 - 1)]; // Drop min & max (basic outlier trimming).
                let total: Duration = trimmed.iter().sum();
                let ave = total / trimmed.len() as u32;

                Ok(Return::Delay(ave))
            };

            loop {
                match execute_cmdrx.try_recv() {
                    Ok(command) => result_tx
                        .send(match command {
                            Command::Tab { x, y } => tab(x, y),
                            Command::Scroll { x1, y1, x2, y2, t } => scroll(x1, y1, x2, y2, t),
                            Command::ControlScreenCapture { start } => {
                                control_screen_capture(start)
                            }
                            Command::TestScreenShotDelay {} => test_screen_shot_delay(),
                        })
                        .expect("WTF"),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        unsafe { lib.nemu_disconnect(connection) };
                        break;
                    }
                };
            }

            info!("Thread Execute End");
        });

        Ok((
            MuMuController {
                execute_cmdtx,
                result_rx,
            },
            screen_capture,
        ))
    }

    #[instrument(skip_all)]
    fn execute(&mut self, command: crate::Command) -> Result<Return> {
        self.execute_cmdtx.send(command).expect("WTF");
        self.result_rx.recv().expect("WTF")
    }
}

#[cfg(test)]
mod tests {

    use anyhow::Ok;

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
