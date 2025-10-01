use std::{path::PathBuf, time::Duration};

use crate::mumu::MuMuController;
use anyhow::{Result, anyhow};
use image::{ImageBuffer, Rgba};
use tokio::{sync::mpsc, task};
use triple_buffer::Output;

mtas_macro::mod_pub!(mumu);

pub enum Platform {
    MuMu,
}

pub enum Controller {
    MuMu(MuMuController),
}

impl Platform {
    fn new(&self) -> Result<(Controller, ScreenCapture)> {
        match self {
            Platform::MuMu => {
                let (controller, screen_capture) = MuMuController::new()?;
                Ok((Controller::MuMu(controller), screen_capture))
            }
        }
    }
}

impl Controller {
    async fn execute(&mut self, command: Command) -> Result<Return> {
        match self {
            Controller::MuMu(controler) => controler.execute(command).await,
        }
    }

    async fn capture_screen(&mut self) -> Result<()> {
        match self {
            Controller::MuMu(controler) => controler.capture_screen().await,
        }
    }

    fn run_loop(
        self,
        mut command_rx: mpsc::Receiver<Command>,
        result_tx: mpsc::Sender<Result<Return, anyhow::Error>>,
    ) {
        task::spawn(async move {
            let mut controller = self;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(16));

            'main_loop: loop {
                tokio::select! {
                    cmd = command_rx.recv() => {
                        match cmd {
                            Some(command) => {
                                let result = controller.execute(command).await;
                                result_tx.send(result).await.expect("WTF");
                            }
                            None => {
                                println!("Command channel fully disconnected, exiting run_loop");
                                break 'main_loop;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if controller.capture_screen().await.is_err() {
                            println!("Screen capture failed, exiting run_loop");
                            break 'main_loop;
                        }
                    }
                }
            }

            println!("run_loop task finished");
        });
    }
}

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
    TestScreenShotDelay {
        iterations: usize,
    },
}

pub struct ScreenCapture {
    pub height: usize,
    pub width: usize,
    pub capture: Output<Vec<u32>>,
}

#[derive(Debug)]
pub enum Return {
    Nothing,
    Delay(Duration),
}

pub(crate) trait ControllerTrait {
    fn new() -> Result<(Self, ScreenCapture)>
    where
        Self: Sized;
    async fn execute(&mut self, command: Command) -> Result<Return>;
    async fn capture_screen(&mut self) -> Result<()>;
}

pub struct ControllerShell {
    command_tx: mpsc::Sender<Command>,
    result_rx: mpsc::Receiver<Result<Return>>,
    screen_capture: ScreenCapture,
}

impl ControllerShell {
    pub fn new(pla: Platform) -> Result<Self> {
        let (controller, screen_capture) = pla.new()?;
        let (command_tx, command_rx) = mpsc::channel(10);
        let (result_tx, result_rx) = mpsc::channel(10);

        controller.run_loop(command_rx, result_tx);

        Ok(Self {
            command_tx,
            result_rx,
            screen_capture,
        })
    }

    pub async fn execute(&mut self, command: Command) -> Result<Return> {
        self.command_tx.send(command).await?;
        self.result_rx.recv().await.expect("WTF")
    }

    pub fn save_screen(&mut self, path: PathBuf) -> Result<()> {
        let screen_cap = &mut self.screen_capture;
        let width = screen_cap.width;
        let height = screen_cap.height;
        let screen_output = screen_cap.capture.read();

        let mut flipped = Vec::with_capacity(screen_output.len());
        for y in (0..height).rev() {
            let start = y * width;
            flipped.extend_from_slice(&screen_output[start..start + width]);
        }
        let data: Vec<u8> = flipped
            .into_iter()
            .flat_map(|pixel| pixel.to_le_bytes())
            .collect();
        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(width as u32, height as u32, data)
                .ok_or_else(|| anyhow!("Failed to create image buffer"))?;
        img.save(path)
            .map_err(|e| anyhow!("Failed to save image: {}", e))?;
        Ok(())
    }
}

pub fn controller(pla: Platform) -> Result<ControllerShell> {
    ControllerShell::new(pla)
}

#[cfg(test)]
mod tests {
    use tokio::time::sleep;

    use super::*;

    #[test]
    fn test_run_loop() -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let mut controller = controller(Platform::MuMu)?;
            println!(
                "{:?}",
                controller
                    .execute(Command::TestScreenShotDelay { iterations: 100 })
                    .await?
            );

            Ok::<(), anyhow::Error>(())
        })?;

        Ok(())
    }

    #[tokio::test]
    async fn test_capture_screen() -> Result<()> {
        let mut controller = controller(Platform::MuMu)?;

        println!(
            "{:?}",
            controller
                .execute(Command::ControlScreenCapture { start: true })
                .await?
        );

        sleep(Duration::from_millis(500)).await; // wait for auto screen_cap finished

        controller.save_screen("./screenshot.png".into())?;

        Ok(())
    }
}
