#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use anyhow::{Ok, Result};
    use dssim_core::Dssim;
    use mtas_controller::{Command, Platform, controller};
    use rgb::FromSlice;

    #[test]
    fn test_dssim() -> Result<()> {
        mtas_logger::init_logger!(std::io::stdout());

        let (mut controller, mut screen_cap) = controller(Platform::MuMu)?;

        controller.execute(Command::ControlScreenCapture { start: true })?;

        let dssim = Dssim::new();
        sleep(Duration::from_millis(100));
        let sc = screen_cap.capture.read().as_rgba();
        let dssim_rgba_1 = dssim
            .create_image_rgba(sc, screen_cap.width, screen_cap.height)
            .unwrap();

        for _ in 0..100 {
            sleep(Duration::from_millis(100));
            let sc = screen_cap.capture.read().as_rgba();
            let dssim_rgba_2 = dssim
                .create_image_rgba(sc, screen_cap.width, screen_cap.height)
                .unwrap();

            tracing::info!("{:?}", dssim.compare(&dssim_rgba_1, dssim_rgba_2).0);
        }

        Ok(())
    }
}
