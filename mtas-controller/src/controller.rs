use std::time::Duration;

use crate::mumu::{MuMuController, MuMuError};
use thiserror::Error;

use image::{ImageBuffer, Rgba};
use triple_buffer::Output;
pub enum Platform {
    MuMu,
}

pub enum Controller {
    MuMu(MuMuController),
}

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("MuMu Controller Error occurred: {0}")]
    MuMuError(#[from] MuMuError),

    #[error("Image Container is Not Big Enough")]
    ScreenCaptureError(),
}

impl Platform {
    fn new(&self) -> Result<(Controller, ScreenCapture), ControllerError> {
        match self {
            Platform::MuMu => {
                let (controller, screen_capture) = MuMuController::new()?;
                Ok((Controller::MuMu(controller), screen_capture))
            }
        }
    }
}

impl Controller {
    pub fn execute(&mut self, command: Command) -> Result<Return, ControllerError> {
        match self {
            Controller::MuMu(controler) => controler.execute(command).map_err(Into::into),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    Tab {
        x: i32,
        y: i32,
    },
    Scroll {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        t: Duration,
    },
    ControlScreenCapture {
        start: bool,
    },
    TestScreenShotDelay {},
}

pub struct ScreenCapture {
    pub height: usize,
    pub width: usize,
    pub capture: Output<Vec<u8>>,
}

impl ScreenCapture {
    pub fn get_screen(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, ControllerError> {
        let width = self.width;
        let height = self.height;
        let img = self.capture.read();

        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(width as u32, height as u32, img.clone())
                .ok_or_else(|| ControllerError::ScreenCaptureError())?;

        Ok(img)
    }
}

#[derive(Debug)]
pub enum Return {
    Nothing,
    Delay(Duration),
}

pub trait ControllerTrait {
    type Error;

    fn new() -> Result<(Self, ScreenCapture), Self::Error>
    where
        Self: Sized;
    fn execute(&mut self, command: Command) -> Result<Return, Self::Error>;
}

pub fn controller(pla: Platform) -> Result<(Controller, ScreenCapture), ControllerError> {
    pla.new()
}

#[cfg(test)]
mod tests {
    use std::{
        thread::{sleep, spawn},
        time::Instant,
    };

    use anyhow::{Result, anyhow};
    use minifb::{Key, Window, WindowOptions};
    use tracing::info;

    use super::*;

    #[test]
    fn test_run_loop() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        let (mut controller, _screen_cap) = controller(Platform::MuMu)?;
        info!("{:?}", controller.execute(Command::TestScreenShotDelay {}));

        Ok(())
    }

    use image::imageops::{self, flip_vertical};

    #[test]
    fn test_capture_screen() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        let (mut controller, mut screen_cap) = controller(Platform::MuMu)?;

        info!("{:?}", controller.execute(Command::TestScreenShotDelay {}));

        sleep(Duration::from_millis(500));

        let img = screen_cap.get_screen()?;

        let img = flip_vertical(&img);

        img.save("./screenshot.png")
            .map_err(|e| anyhow!("Failed to save image: {}", e))?;

        let gray = imageops::grayscale(&img);

        gray.save("./screenshot_gray.png")
            .map_err(|e| anyhow!("Failed to save image: {}", e))?;

        Ok(())
    }

    #[test]
    fn test_vedio_speed() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        let (mut controller, mut screen_cap) = controller(Platform::MuMu)?;

        info!("{:?}", controller.execute(Command::TestScreenShotDelay {}));

        let mut times = Vec::with_capacity(10);

        for _ in 0..10 {
            let start = Instant::now();
            'innerloop: loop {
                if screen_cap.capture.update() {
                    times.push(start.elapsed());
                    break 'innerloop;
                }
            }
        }

        times.sort();
        let trimmed = &times[1..(10 - 1)]; // Drop min & max (basic outlier trimming).
        let total: Duration = trimmed.iter().sum();
        let ave = total / trimmed.len() as u32;

        info!("Average capture time: {:?}", ave);

        Ok(())
    }

    #[test]
    fn test_vedio_show() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        let (mut controller, mut screen_cap) = controller(Platform::MuMu)?;

        info!("{:?}", controller.execute(Command::TestScreenShotDelay {}));

        let width = screen_cap.width;
        let height = screen_cap.height;

        spawn(move || {
            let mut window =
                Window::new("vedio", width, height, WindowOptions::default()).expect("WTF");

            window.set_target_fps(120);

            while window.is_open() && !window.is_key_down(Key::Escape) {
                let buffer_u8 = screen_cap.capture.read();

                let buffer_u32_slice = bytemuck::cast_slice::<u8, u32>(&buffer_u8);

                if buffer_u32_slice.len() != width * height {
                    info!(
                        "Warning: Buffer length mismatch. Expected: {}, Got: {}. Skipping frame.",
                        width * height,
                        buffer_u32_slice.len()
                    );
                    continue;
                }

                window
                    .update_with_buffer(&buffer_u32_slice, width, height)
                    .unwrap();
            }
        })
        .join()
        .map_err(|e| anyhow!("video thread panicked: {:?}", e))?;

        Ok(())
    }

    #[test]
    fn test_command_sequence_performance() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        let (mut controller, _screen_cap) = controller(Platform::MuMu)?;

        let commands = vec![
            Command::Scroll {
                x1: 100,
                y1: 500,
                x2: 1000,
                y2: 200,
                t: Duration::from_millis(100),
            },
            Command::Scroll {
                x1: 100,
                y1: 500,
                x2: 1000,
                y2: 200,
                t: Duration::from_millis(100),
            },
            Command::Scroll {
                x1: 100,
                y1: 500,
                x2: 1000,
                y2: 200,
                t: Duration::from_millis(100),
            },
        ];

        // --- 1. Benchmark without screen capture ---
        controller.execute(Command::ControlScreenCapture { start: false })?;
        sleep(Duration::from_millis(100));

        let start_no_capture = Instant::now();
        for cmd in &commands {
            controller.execute(cmd.clone())?;
        }
        let duration_no_capture = start_no_capture.elapsed();
        println!("\n--- Performance Test Results ---");
        println!("Without screen capture: {:?}", duration_no_capture);

        // --- 2. Benchmark with screen capture ---
        controller.execute(Command::ControlScreenCapture { start: true })?;
        sleep(Duration::from_millis(100));

        let start_with_capture = Instant::now();
        for cmd in &commands {
            controller.execute(cmd.clone())?;
        }
        let duration_with_capture = start_with_capture.elapsed();
        println!("With screen capture:    {:?}", duration_with_capture);
        println!("------------------------------\n");

        Ok(())
    }
}
