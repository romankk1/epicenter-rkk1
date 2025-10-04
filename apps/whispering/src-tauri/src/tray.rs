use std::sync::{Arc, Mutex};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId},
    AppHandle, Manager, Runtime,
};

/// Manages the system tray icon and its state
pub struct TrayManager {
    is_recording: Arc<Mutex<bool>>,
    close_to_tray: Arc<Mutex<bool>>,
    start_minimized: Arc<Mutex<bool>>,
    tray_icon_id: Arc<Mutex<Option<TrayIconId>>>,
}

impl TrayManager {
    /// Creates a new tray manager with idle state
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            close_to_tray: Arc::new(Mutex::new(false)),
            start_minimized: Arc::new(Mutex::new(false)),
            tray_icon_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Updates the recording state and tray icon
    pub fn set_recording_state(&self, recording: bool) {
        if let Ok(mut is_recording) = self.is_recording.lock() {
            *is_recording = recording;
            // Note: Icon update will be handled by the tray icon update method
        }
    }

    /// Gets the current recording state
    pub fn is_recording(&self) -> bool {
        self.is_recording.lock().map(|guard| *guard).unwrap_or(false)
    }

    /// Updates tray behavior settings
    pub fn update_settings(&self, close_to_tray: bool, start_minimized: bool) {
        if let Ok(mut close_setting) = self.close_to_tray.lock() {
            *close_setting = close_to_tray;
        }
        if let Ok(mut minimized_setting) = self.start_minimized.lock() {
            *minimized_setting = start_minimized;
        }
    }

    /// Gets the close to tray setting
    pub fn should_close_to_tray(&self) -> bool {
        self.close_to_tray.lock().map(|guard| *guard).unwrap_or(false)
    }

    /// Shows the main application window
    pub fn show_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = app.get_webview_window("main") {
            window.show()?;
            window.set_focus()?;
        }
        Ok(())
    }

    /// Hides the main application window
    pub fn hide_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = app.get_webview_window("main") {
            window.hide()?;
        }
        Ok(())
    }
}

/// Initializes the system tray with menu and event handlers
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Create tray menu
    let show_item = MenuItem::with_id(app, "show", "Show Whispering", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    // Use the default window icon for now (we'll improve this later)
    let icon = app.default_window_icon()
        .ok_or("No default window icon available")?
        .clone();
    
    // Build tray icon
    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(icon)
        .tooltip("Whispering - Idle")
        .on_tray_icon_event(|tray, event| {
            handle_tray_event(tray.app_handle(), event);
        })
        .on_menu_event(|app, event| {
            handle_menu_event(app, event);
        })
        .build(app)?;

    // Store tray icon ID in TrayManager if available
    if let Some(tray_manager) = app.try_state::<TrayManager>() {
        if let Ok(mut tray_icon_id) = tray_manager.tray_icon_id.lock() {
            *tray_icon_id = Some(tray.id().clone());
        }
    }



    Ok(())
}

/// Handles tray icon events (clicks, menu selections)
fn handle_tray_event<R: Runtime>(app: &AppHandle<R>, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            // Left click toggles window visibility
            if let Some(window) = app.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = TrayManager::hide_window(app);
                } else {
                    let _ = TrayManager::show_window(app);
                }
            }
        }
        _ => {
            // Handle other events as needed
            tracing::debug!("Unhandled tray event: {:?}", event);
        }
    }
}

/// Handles menu events from the tray
fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    tracing::info!("Tray menu event: {:?}", event.id());
    
    match event.id().as_ref() {
        "show" => {
            let _ = TrayManager::show_window(app);
        }
        "quit" => {
            app.exit(0);
        }
        _ => {
            tracing::debug!("Unhandled menu event: {:?}", event.id());
        }
    }
}

/// Updates the tray icon based on app state
pub fn update_tray_icon<R: Runtime>(
    app: &AppHandle<R>,
    state: TrayState,
) -> Result<(), Box<dyn std::error::Error>> {
    let (icon_path, tooltip) = get_tray_info(state);

    // Try to get the tray manager and tray icon ID
    if let Some(tray_manager) = app.try_state::<TrayManager>() {
        if let Ok(tray_icon_id_guard) = tray_manager.tray_icon_id.lock() {
            if let Some(tray_icon_id) = tray_icon_id_guard.as_ref() {
                // Get tray icon from app's tray collection
                if let Some(tray_icon) = app.tray_by_id(tray_icon_id) {
                    // Update tooltip
                    let _ = tray_icon.set_tooltip(Some(tooltip));
                    
                    // Try to load and update icon
                    if let Ok(icon_data) = std::fs::read(icon_path) {
                        if let Ok(icon) = Image::from_bytes(&icon_data) {
                            let _ = tray_icon.set_icon(Some(icon));
                            tracing::info!("Tray icon updated: {} ({})", tooltip, icon_path);
                        } else {
                            tracing::warn!("Failed to parse icon from {}", icon_path);
                        }
                    } else {
                        tracing::warn!("Failed to load icon file: {}", icon_path);
                    }
                    return Ok(());
                }
            }
        }
    }
    
    tracing::info!("Tray state updated (icon not available): {}", tooltip);
    Ok(())
}

/// Tray states for different app operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayState {
    Idle,
    Recording,
    Processing,
}

/// Returns the appropriate icon path and tooltip based on tray state
fn get_tray_info(state: TrayState) -> (&'static str, &'static str) {
    match state {
        TrayState::Idle => ("icons/tray-idle.png", "Whispering - Idle"),
        TrayState::Recording => ("icons/tray-recording.png", "Whispering - Recording"),
        TrayState::Processing => ("icons/tray-processing.png", "Whispering - Processing"),
    }
}

/// Tauri command to update tray recording state from frontend
#[tauri::command]
pub fn update_tray_recording_state<R: Runtime>(
    recording: bool,
    app: AppHandle<R>,
    tray_manager: tauri::State<TrayManager>,
) -> Result<(), String> {
    // Update the internal state
    tray_manager.set_recording_state(recording);
    
    // Update the tray icon
    let state = if recording { TrayState::Recording } else { TrayState::Idle };
    update_tray_icon(&app, state).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Tauri command to update tray processing state from frontend
#[tauri::command]
pub fn update_tray_processing_state<R: Runtime>(
    processing: bool,
    app: AppHandle<R>,
) -> Result<(), String> {
    // Update the tray icon to processing or idle state
    let state = if processing { TrayState::Processing } else { TrayState::Idle };
    update_tray_icon(&app, state).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Tauri command to check if system tray is supported
#[tauri::command]
pub fn is_tray_supported() -> bool {
    // System tray support varies by platform and desktop environment
    // For now, we'll assume it's supported and handle errors gracefully
    true
}

/// Tauri command to show/hide window from frontend
#[tauri::command]
pub fn toggle_window_visibility<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().map_err(|e| e.to_string())? {
            TrayManager::hide_window(&app).map_err(|e| e.to_string())?;
        } else {
            TrayManager::show_window(&app).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Tauri command to set tray behavior settings from frontend
#[tauri::command]
pub fn set_tray_settings(
    close_to_tray: bool,
    start_minimized: bool,
    tray_manager: tauri::State<TrayManager>,
) -> Result<(), String> {
    tray_manager.update_settings(close_to_tray, start_minimized);
    tracing::info!("Tray settings updated: close_to_tray={}, start_minimized={}", close_to_tray, start_minimized);
    Ok(())
}

/// Checks if window should hide to tray instead of closing
pub fn should_hide_to_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<TrayManager>()
        .map(|tray_manager| tray_manager.should_close_to_tray())
        .unwrap_or(false)
}

