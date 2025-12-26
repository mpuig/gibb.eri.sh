//! Example: Poll system context and print changes.
//!
//! Run with: cargo run -p gibberish-context --example poll_context

use gibberish_context::{platform::PlatformProvider, ContextPoller};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("gibberish_context=debug")
        .init();

    println!("=== Context Poller Example ===");
    println!("Polling system context every 1.5 seconds...");
    println!("Switch between apps to see mode changes.\n");

    // Create the platform-specific provider
    let provider = Arc::new(PlatformProvider::new());

    // Create and start the poller
    let mut poller = ContextPoller::new();

    poller.start(
        provider,
        Arc::new(|event| {
            println!(
                "[{}] Mode: {:6} | App: {} | Meeting: {}",
                chrono::Local::now().format("%H:%M:%S"),
                event.mode,
                event
                    .active_app_name
                    .as_deref()
                    .or(event.active_app.as_deref())
                    .unwrap_or("(none)"),
                if event.is_meeting { "yes" } else { "no" }
            );
        }),
    );

    // Run for 30 seconds
    println!("Running for 30 seconds... (Ctrl+C to stop)\n");
    std::thread::sleep(Duration::from_secs(30));

    poller.stop();
    println!("\nDone.");
}
