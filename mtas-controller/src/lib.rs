use std::time::Duration;

use crate::mumu::MuMuController;
use anyhow::Result;
use tokio::{sync::mpsc, task};

mtas_macro::mod_pub!(mumu);

pub enum Platform {
    MuMu,
}

pub enum Controller {
    MuMu(MuMuController),
}

impl Platform {
    pub fn new(&self) -> Result<Controller> {
        match self {
            Platform::MuMu => Ok(Controller::MuMu(MuMuController::new()?)),
        }
    }
}

impl Controller {
    fn execute(&mut self, command: Command) -> Result<Return> {
        match self {
            Controller::MuMu(controler) => controler.execute(command),
        }
    }

    fn capture_screen(&mut self) -> Result<()> {
        match self {
            Controller::MuMu(controler) => controler.capture_screen(),
        }
    }

    pub fn run_loop(
        self,
        mut command_rx: mpsc::Receiver<Command>,
        result_tx: mpsc::Sender<Result<Return, anyhow::Error>>,
    ) -> task::JoinHandle<()> {
        task::spawn_blocking(move || {
            let mut controller = self;
            loop {
                match command_rx.try_recv() {
                    Ok(cmd) => {
                        let result = controller.execute(cmd);
                        let _ = result_tx.try_send(result);
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        if let Err(err) = controller.capture_screen() {
                            let _ = result_tx.try_send(Err(err));
                        }
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        break;
                    }
                }
            }
        })
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
    TestScreenShotDelay {
        iterations: usize,
    },
}

#[derive(Debug)]
pub enum Return {
    Nothing,
    Delay(Duration),
}

pub(crate) trait ControllerTrait {
    fn new() -> Result<Self>
    where
        Self: Sized;
    fn execute(&mut self, command: Command) -> Result<Return>;
    fn capture_screen(&mut self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_loop() -> Result<()> {
        let controller = Platform::MuMu.new()?;
        let (command_tx, command_rx) = mpsc::channel(10);
        let (result_tx, mut result_rx) = mpsc::channel(10);

        let handle = controller.run_loop(command_rx, result_tx);

        // command_tx.send(Command::Tab { x: 200, y: 1000 }).await?;
        // let result = result_rx.recv().await.unwrap();
        // println!("{:?}", result);

        command_tx
            .send(Command::TestScreenShotDelay { iterations: 100 })
            .await?;
        let result = result_rx.recv().await.unwrap();
        println!("{:?}", result);

        drop(command_tx);

        handle.await?;

        Ok(())
    }
}
