use anyhow::Result;
use mtas_controller::{ControllerShell, Platform, controller};

pub struct Executer {
    controller: ControllerShell,
}

impl Executer {
    pub fn new(pla: Platform) -> Result<Self> {
        Ok(Executer {
            controller: controller(pla)?,
        })
    }
}

pub fn executer(pla: Platform) -> Result<Executer> {
    Executer::new(pla)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn simple_test() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
