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
    fn new(&self) -> Result<Controller> {
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

    fn run_loop(
        self,
        mut command_rx: mpsc::Receiver<Command>,
        result_tx: mpsc::Sender<Result<Return, anyhow::Error>>,
    ) {
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

pub struct ControllerShell {
    command_tx: mpsc::Sender<Command>,
    result_rx: mpsc::Receiver<Result<Return>>,
}

impl ControllerShell {
    pub fn new(pla: Platform) -> Result<Self> {
        let controller = pla.new()?;
        let (command_tx, command_rx) = mpsc::channel(10);
        let (result_tx, result_rx) = mpsc::channel(10);

        controller.run_loop(command_rx, result_tx);

        Ok(Self {
            command_tx,
            result_rx,
        })
    }

    pub async fn execute(&mut self, command: Command) -> Result<Return> {
        self.command_tx.send(command).await?;
        self.result_rx.recv().await.expect("WTF")
    }
}

pub fn controller(pla: Platform) -> Result<ControllerShell> {
    ControllerShell::new(pla)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_loop() -> Result<()> {
        let mut controller = controller(Platform::MuMu)?;

        println!(
            "{:?}",
            controller
                .execute(Command::TestScreenShotDelay { iterations: 100 })
                .await?
        );

        Ok(())
    }
}
