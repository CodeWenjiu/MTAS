use crate::mumu::MuMuController;
use anyhow::{Error, Result};
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
    pub fn execute(&mut self, command: Command) -> Result<()> {
        match self {
            Controller::MuMu(controler) => controler.execute(command),
        }
    }

    pub fn capture_screen(&mut self) -> Result<()> {
        match self {
            Controller::MuMu(controler) => controler.capture_screen(),
        }
    }

    pub fn run_loop(
        self,
        mut command_rx: mpsc::Receiver<Command>,
        error_tx: mpsc::Sender<Error>,
    ) -> task::JoinHandle<()> {
        task::spawn_blocking(move || {
            let mut controller = self;
            loop {
                match command_rx.try_recv() {
                    Ok(cmd) => {
                        if let Err(e) = controller.execute(cmd) {
                            let _ = error_tx.try_send(e);
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        if let Err(e) = controller.capture_screen() {
                            let _ = error_tx.try_send(e);
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
    Tab { x: i32, y: i32 },
    Scroll { x1: i32, y1: i32, x2: i32, y2: i32 },
}

pub(crate) trait ControllerTrait {
    fn new() -> Result<Self>
    where
        Self: Sized;
    fn execute(&mut self, command: Command) -> Result<()>;
    fn capture_screen(&mut self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_loop() {
        let controller = Platform::MuMu.new().unwrap();
        let (command_tx, command_rx) = mpsc::channel(10);
        let (error_tx, mut error_rx) = mpsc::channel(10);

        let handle = controller.run_loop(command_rx, error_tx);

        // Send a command
        command_tx
            .send(Command::Tab { x: 200, y: 1000 })
            .await
            .unwrap();

        // Receive result
        let result = error_rx.try_recv();
        assert!(result.is_err());

        // Close the command channel
        drop(command_tx);

        // Wait for the loop to finish
        handle.await.unwrap();
    }
}
