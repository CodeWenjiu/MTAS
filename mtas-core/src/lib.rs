#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mtas_controller::{Command, Platform, controller};
    use tokio::time::sleep;

    #[tokio::test]
    async fn simple_test() -> Result<(), Box<dyn std::error::Error>> {
        println!("MTAS - Millennium Tour Auto Script");

        let mut controller = controller(Platform::MuMu)?;

        let _ = controller
            .execute(Command::ControlScreenCapture { start: true })
            .await?;

        sleep(Duration::from_millis(500)).await; // wait for auto screen_cap finished

        controller.save_screen("./screenshot.png".into())?;

        Ok(())
    }
}
