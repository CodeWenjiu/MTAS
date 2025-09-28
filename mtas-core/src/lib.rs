#[cfg(test)]
mod tests {
    use mtas_adb::ADBDevice;

    #[tokio::test]
    async fn test_connect() -> Result<(), Box<dyn std::error::Error>> {
        async fn test_command(tx: tokio::sync::mpsc::Sender<Vec<String>>) -> Result<(), Box<dyn std::error::Error>> {
            let command_to_send = vec![
                "input".to_string(),
                "tap".to_string(),
                "2000".to_string(),
                "500".to_string(),
            ];
            tx.send(command_to_send).await?;
            drop(tx);
            Ok(())
        }

        let adb = ADBDevice::new(mtas_adb::Platform::MuMu)?;

        let (adb_task, tx) = adb.into_command_handler();

        test_command(tx).await?;

        adb_task.await??;

        Ok(())
    }
}