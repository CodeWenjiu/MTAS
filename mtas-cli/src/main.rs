use std::{thread::sleep, time::Duration};

use mtas_controller::{Command, Platform, controller};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MTAS - Millennium Tour Auto Script");

    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        let mut controller = controller(Platform::MuMu)?;
        controller
            .execute(Command::ControlScreenCapture { start: true })
            .await?;

        sleep(Duration::from_millis(500));

        controller.save_screen("./screenshot.png".into())?;

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}
