#[cfg(target_os = "macos")]
mod macos {
    use crate::{list_mic_using_apps, BackgroundTask, DetectCallback, DetectEvent, InstalledApp};
    use cidre::{core_audio as ca, os};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    const DEVICE_IS_RUNNING_SOMEWHERE: ca::PropAddr = ca::PropAddr {
        selector: ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE,
        scope: ca::PropScope::GLOBAL,
        element: ca::PropElement::MAIN,
    };

    const POLL_INTERVAL: Duration = Duration::from_secs(1);

    #[derive(Default)]
    pub struct Detector {
        background: BackgroundTask,
    }

    struct DetectorState {
        last_state: bool,
        last_change: Instant,
        debounce_duration: Duration,
        active_apps: Vec<InstalledApp>,
    }

    impl DetectorState {
        fn new() -> Self {
            Self {
                last_state: false,
                last_change: Instant::now(),
                debounce_duration: Duration::from_millis(500),
                active_apps: Vec::new(),
            }
        }

        fn should_trigger(&mut self, new_state: bool) -> bool {
            let now = Instant::now();
            if new_state == self.last_state {
                return false;
            }
            if now.duration_since(self.last_change) < self.debounce_duration {
                return false;
            }
            self.last_state = new_state;
            self.last_change = now;
            true
        }
    }

    struct SharedContext {
        callback: Arc<Mutex<DetectCallback>>,
        current_device: Arc<Mutex<Option<ca::Device>>>,
        state: Arc<Mutex<DetectorState>>,
        polling_active: Arc<AtomicBool>,
        /// Shutdown signal for all threads.
        shutdown: Arc<AtomicBool>,
    }

    impl SharedContext {
        fn new(callback: DetectCallback) -> Self {
            Self {
                callback: Arc::new(Mutex::new(callback)),
                current_device: Arc::new(Mutex::new(None)),
                state: Arc::new(Mutex::new(DetectorState::new())),
                polling_active: Arc::new(AtomicBool::new(false)),
                shutdown: Arc::new(AtomicBool::new(false)),
            }
        }

        fn clone_shared(&self) -> Self {
            Self {
                callback: self.callback.clone(),
                current_device: self.current_device.clone(),
                state: self.state.clone(),
                polling_active: self.polling_active.clone(),
                shutdown: self.shutdown.clone(),
            }
        }

        fn is_shutdown(&self) -> bool {
            self.shutdown.load(Ordering::SeqCst)
        }

        fn emit(&self, event: DetectEvent) {
            tracing::info!(?event, "detected");
            if let Ok(guard) = self.callback.lock() {
                (*guard)(event);
            }
        }

        fn handle_mic_change(&self, mic_in_use: bool) {
            let Ok(mut state_guard) = self.state.lock() else {
                return;
            };

            if !state_guard.should_trigger(mic_in_use) {
                return;
            }

            if mic_in_use {
                let apps = list_mic_using_apps();
                state_guard.active_apps = apps.clone();
                self.polling_active.store(true, Ordering::SeqCst);
                drop(state_guard);
                self.emit(DetectEvent::MicStarted(apps));
            } else {
                self.polling_active.store(false, Ordering::SeqCst);
                let stopped_apps = std::mem::take(&mut state_guard.active_apps);
                drop(state_guard);
                self.emit(DetectEvent::MicStopped(stopped_apps));
            }
        }
    }

    fn is_mic_running(device: &ca::Device) -> bool {
        device
            .prop::<u32>(&DEVICE_IS_RUNNING_SOMEWHERE)
            .map(|v| v != 0)
            .unwrap_or(false)
    }

    fn diff_apps(
        previous: &[InstalledApp],
        current: &[InstalledApp],
    ) -> (Vec<InstalledApp>, Vec<InstalledApp>) {
        let previous_ids: HashSet<_> = previous.iter().map(|app| &app.id).collect();
        let current_ids: HashSet<_> = current.iter().map(|app| &app.id).collect();

        let started = current
            .iter()
            .filter(|app| !previous_ids.contains(&app.id))
            .cloned()
            .collect();

        let stopped = previous
            .iter()
            .filter(|app| !current_ids.contains(&app.id))
            .cloned()
            .collect();

        (started, stopped)
    }

    struct ListenerData {
        ctx: SharedContext,
        device_listener_ptr: *mut (),
    }

    fn spawn_polling_thread(ctx: SharedContext) {
        std::thread::spawn(move || {
            while !ctx.is_shutdown() {
                std::thread::sleep(POLL_INTERVAL);

                if ctx.is_shutdown() {
                    break;
                }

                if !ctx.polling_active.load(Ordering::SeqCst) {
                    continue;
                }

                let current_apps = list_mic_using_apps();
                let Ok(mut state_guard) = ctx.state.lock() else {
                    continue;
                };

                let (started, stopped) = diff_apps(&state_guard.active_apps, &current_apps);
                state_guard.active_apps = current_apps;
                drop(state_guard);

                if !started.is_empty() {
                    let event = DetectEvent::MicStarted(started);
                    tracing::info!(?event, "detected_via_polling");
                    if let Ok(guard) = ctx.callback.lock() {
                        (*guard)(event);
                    }
                }

                if !stopped.is_empty() {
                    let event = DetectEvent::MicStopped(stopped);
                    tracing::info!(?event, "detected_via_polling");
                    if let Ok(guard) = ctx.callback.lock() {
                        (*guard)(event);
                    }
                }
            }
            tracing::debug!("polling thread exiting");
        });
    }

    extern "C-unwind" fn device_listener(
        _obj_id: ca::Obj,
        number_addresses: u32,
        addresses: *const ca::PropAddr,
        client_data: *mut (),
    ) -> os::Status {
        let data = unsafe { &*(client_data as *const ListenerData) };
        let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

        for addr in addresses {
            if addr.selector != ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE {
                continue;
            }
            if let Ok(device) = ca::System::default_input_device() {
                data.ctx.handle_mic_change(is_mic_running(&device));
            }
        }

        os::Status::NO_ERR
    }

    extern "C-unwind" fn system_listener(
        _obj_id: ca::Obj,
        number_addresses: u32,
        addresses: *const ca::PropAddr,
        client_data: *mut (),
    ) -> os::Status {
        let data = unsafe { &*(client_data as *const ListenerData) };
        let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

        for addr in addresses {
            if addr.selector != ca::PropSelector::HW_DEFAULT_INPUT_DEVICE {
                continue;
            }

            let Ok(mut device_guard) = data.ctx.current_device.lock() else {
                continue;
            };

            if let Some(old_device) = device_guard.take() {
                let _ = old_device.remove_prop_listener(
                    &DEVICE_IS_RUNNING_SOMEWHERE,
                    device_listener,
                    data.device_listener_ptr,
                );
            }

            let Ok(new_device) = ca::System::default_input_device() else {
                continue;
            };

            if new_device
                .add_prop_listener(
                    &DEVICE_IS_RUNNING_SOMEWHERE,
                    device_listener,
                    data.device_listener_ptr,
                )
                .is_ok()
            {
                let mic_in_use = is_mic_running(&new_device);
                *device_guard = Some(new_device);
                drop(device_guard);
                data.ctx.handle_mic_change(mic_in_use);
            }
        }

        os::Status::NO_ERR
    }

    impl crate::Observer for Detector {
        fn start(&mut self, f: DetectCallback) {
            self.background.start(|running, mut rx| async move {
                let (ready_tx, mut ready_rx) = tokio::sync::mpsc::channel::<()>(1);
                let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

                // Clone running flag for the listener thread
                let thread_running = running.clone();

                std::thread::spawn(move || {
                    let ctx = SharedContext::new(f);

                    spawn_polling_thread(ctx.clone_shared());

                    let device_listener_data = Box::new(ListenerData {
                        ctx: ctx.clone_shared(),
                        device_listener_ptr: std::ptr::null_mut(),
                    });
                    let device_listener_ptr = Box::into_raw(device_listener_data) as *mut ();

                    let system_listener_data = Box::new(ListenerData {
                        ctx: ctx.clone_shared(),
                        device_listener_ptr,
                    });
                    let system_listener_ptr = Box::into_raw(system_listener_data) as *mut ();

                    if let Err(e) = ca::System::OBJ.add_prop_listener(
                        &ca::PropSelector::HW_DEFAULT_INPUT_DEVICE.global_addr(),
                        system_listener,
                        system_listener_ptr,
                    ) {
                        tracing::error!("adding_system_listener_failed: {:?}", e);
                    } else {
                        tracing::info!("adding_system_listener_success");
                    }

                    match ca::System::default_input_device() {
                        Ok(device) => {
                            let mic_in_use = is_mic_running(&device);
                            if device
                                .add_prop_listener(
                                    &DEVICE_IS_RUNNING_SOMEWHERE,
                                    device_listener,
                                    device_listener_ptr,
                                )
                                .is_ok()
                            {
                                tracing::info!("adding_device_listener_success");

                                let data =
                                    unsafe { &*(system_listener_ptr as *const ListenerData) };
                                if let Ok(mut device_guard) = data.ctx.current_device.lock() {
                                    *device_guard = Some(device);
                                }
                                if let Ok(mut state_guard) = data.ctx.state.lock() {
                                    state_guard.last_state = mic_in_use;
                                    if mic_in_use {
                                        state_guard.active_apps = list_mic_using_apps();
                                        data.ctx.polling_active.store(true, Ordering::SeqCst);
                                    }
                                }
                            } else {
                                tracing::error!("adding_device_listener_failed");
                            }
                        }
                        Err(_) => tracing::warn!("no_default_input_device_found"),
                    }

                    // Signal that setup is complete
                    let _ = ready_tx.blocking_send(());

                    // Wait for shutdown signal (poll instead of parking forever)
                    while thread_running.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(100));
                    }

                    // Signal shutdown to polling thread
                    ctx.shutdown.store(true, Ordering::SeqCst);

                    // Clean up listeners
                    tracing::debug!("cleaning up mic detector listeners");

                    // Remove system listener
                    let _ = ca::System::OBJ.remove_prop_listener(
                        &ca::PropSelector::HW_DEFAULT_INPUT_DEVICE.global_addr(),
                        system_listener,
                        system_listener_ptr,
                    );

                    // Remove device listener if device is set
                    if let Ok(device_guard) = ctx.current_device.lock() {
                        if let Some(ref device) = *device_guard {
                            let _ = device.remove_prop_listener(
                                &DEVICE_IS_RUNNING_SOMEWHERE,
                                device_listener,
                                device_listener_ptr,
                            );
                        }
                    }

                    // Free the Box'd data
                    unsafe {
                        drop(Box::from_raw(system_listener_ptr as *mut ListenerData));
                        drop(Box::from_raw(device_listener_ptr as *mut ListenerData));
                    }

                    tracing::debug!("mic detector listener thread exiting");
                    let _ = done_tx.blocking_send(());
                });

                // Wait for setup to complete
                let _ = ready_rx.recv().await;

                // Wait for stop signal
                loop {
                    tokio::select! {
                        _ = &mut rx => break,
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {
                            if !running.load(Ordering::SeqCst) {
                                break;
                            }
                        }
                    }
                }

                // Wait for thread cleanup to complete (with timeout)
                let _ = tokio::time::timeout(Duration::from_secs(2), done_rx.recv()).await;
            });
        }

        fn stop(&mut self) {
            self.background.stop();
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::Detector as MicDetector;

#[cfg(not(target_os = "macos"))]
pub struct MicDetector;

#[cfg(not(target_os = "macos"))]
impl Default for MicDetector {
    fn default() -> Self {
        Self
    }
}

#[cfg(not(target_os = "macos"))]
impl crate::Observer for MicDetector {
    fn start(&mut self, _f: crate::DetectCallback) {}
    fn stop(&mut self) {}
}
