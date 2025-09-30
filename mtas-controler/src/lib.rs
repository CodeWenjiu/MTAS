use crate::mumu::MuMuControler;

mtas_macro::mod_pub!(mumu);

pub enum Platform {
    MuMu,
}

pub enum Controller {
    MuMu(MuMuControler),
}

impl Platform {
    pub fn new(&self) -> Controller {
        match self {
            Platform::MuMu => Controller::MuMu(MuMuControler::new()),
        }
    }
}

pub enum Command {
    Tab { x: i32, y: i32 },
    ScreenCap,
}

pub(crate) trait controller_trait {
    fn new() -> Self;
    fn execute(&mut self, command: Command);
}
