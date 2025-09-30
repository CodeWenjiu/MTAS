use crate::mumu::MuMuController;
use anyhow::Result;

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
}

pub enum Command {
    Tab { x: i32, y: i32 },
    Scroll { x1: i32, y1: i32, x2: i32, y2: i32 },
    ScreenCap,
}

pub(crate) trait ControllerTrait {
    fn new() -> Result<Self>
    where
        Self: Sized;
    fn execute(&mut self, command: Command) -> Result<()>;
}
