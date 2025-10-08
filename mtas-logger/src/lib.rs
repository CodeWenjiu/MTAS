use std::{fmt::Debug, io::Write};

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;

#[tracing::instrument]
/// Initialize the global tracing subscriber/logger.
///
/// This function is intended to be called exactly once at application start (e.g. in main)
/// before any logging occurs. It installs a global default subscriber.
///
/// The returned WorkerGuard MUST be kept alive (not dropped) for as long as you want
/// logging to function correctly. Dropping it will stop the background logging worker
/// and may cause log events to be lost.
///
/// Typical usage:
/// ```rust
/// fn main() {
///     let _guard = mtas_logger::set_logger(std::io::stdout())?;
///     info!("Hello, world!");
///     ...
/// }
/// ```
/// (Store `_guard` in a variable you keep until shutdown; using a leading underscore
/// silences unused warnings while still preserving the guard.)
pub fn set_logger<T: Write + Send + Debug + 'static>(writer: T) -> Result<WorkerGuard> {
    let (append, _guard) = tracing_appender::non_blocking(writer);

    let subscriber = tracing_subscriber::fmt()
        // Output to stdout
        .with_writer(append)
        // Use a more pretty, human-readable log format
        .pretty()
        // Use ANSI colors for output
        .with_ansi(true)
        // Dont display the timestamp
        .without_time()
        // Display source code file paths
        .with_file(true)
        // Display source code line numbers
        .with_line_number(true)
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        // Don't display the event's target (module path)
        .with_target(false)
        // Build the subscriber
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    tracing::info!("Logger initialized");

    Ok(_guard)
}

#[macro_export]
/// Convenience macro to initialize the global logger/tracing subscriber.
///
/// This is a thin wrapper around `set_logger` intended for ergonomic use in `main`
/// or test setup code. It creates and stores the required `WorkerGuard` in a
/// local variable named `_guard`, which must live for as long as you need logging
/// to function. Dropping the guard will stop the background logging worker,
/// potentially causing log events to be lost.
///
/// Usage:
/// ```rust,no_run
/// fn main() -> anyhow::Result<()> {
///     mtas_logger::init_logger!(std::io::stdout());
///     tracing::info!("Application started");
///     Ok(())
/// }
/// ```
///
/// You may pass any writer that implements `Write + Send + Debug + 'static`,
/// such as `std::io::stdout()`, a file, or an in-memory buffer.
///
/// Note:
/// - The underscore in `_guard` suppresses unused variable warnings while
///   still keeping the guard alive.
/// - This macro returns `Result<()>` context via the `?` operator within the
///   calling function (so your function should return a compatible `Result`).
macro_rules! init_logger {
    ($writer:expr) => {
        let _guard = $crate::set_logger($writer)?;
    };
}
