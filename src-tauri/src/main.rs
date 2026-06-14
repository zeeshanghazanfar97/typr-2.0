#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::{env, fs};
use tauri::{
    Emitter, LogicalPosition, LogicalSize, Manager, Position, Size, State, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use typr_lib::audio;
use typr_lib::downloader;
use typr_lib::history::{self, TranscriptHistoryEntry};
use typr_lib::recorder::{Recorder, RecordingState};
use typr_lib::settings::Settings;
use typr_lib::transcribe_local;

struct AppState {
    recorder: Recorder,
    settings: Mutex<Settings>,
    app_dir: PathBuf,
    modifier_hotkey_watcher: Mutex<Option<ModifierHotkeyWatcher>>,
}

struct ModifierHotkeyWatcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
struct OverlayWindowPreset {
    width: f64,
    height: f64,
}

// Keep these presets close to src/overlay.css dock variables when changing size.
const OVERLAY_PARKED: OverlayWindowPreset = OverlayWindowPreset {
    width: 120.0,
    height: 44.0,
};
const OVERLAY_DOCK: OverlayWindowPreset = OverlayWindowPreset {
    width: 460.0,
    height: 86.0,
};
const OVERLAY_LABEL: OverlayWindowPreset = OverlayWindowPreset {
    width: 460.0,
    height: 144.0,
};
const OVERLAY_MENU: OverlayWindowPreset = OverlayWindowPreset {
    width: 460.0,
    height: 420.0,
};
const OVERLAY_RECORDING: OverlayWindowPreset = OverlayWindowPreset {
    width: 440.0,
    height: 92.0,
};
const OVERLAY_STATUS: OverlayWindowPreset = OverlayWindowPreset {
    width: 360.0,
    height: 76.0,
};
const OVERLAY_EDGE_MARGIN: f64 = 16.0;
const OVERLAY_DOCK_GAP: f64 = 4.0;

const NOTES_WIDTH: f64 = 380.0;
const NOTES_HEIGHT: f64 = 480.0;

/// Cursor position relative to the overlay window, in logical (CSS) pixels.
#[derive(Clone, serde::Serialize)]
struct OverlayCursorPayload {
    x: f64,
    y: f64,
    inside: bool,
}

/// Polls the global cursor and streams its position to the overlay webview.
///
/// macOS does not deliver hover events to an unfocused WKWebView, so the
/// overlay cannot rely on DOM mouseenter/mousemove until it has been clicked.
/// This watcher works regardless of focus and lets the webview drive all
/// hover states (expand, tooltips, button highlights) itself.
fn start_overlay_cursor_watcher(app: tauri::AppHandle) {
    const INSIDE_POLL_MS: u64 = 33;
    const OUTSIDE_POLL_MS: u64 = 90;

    std::thread::spawn(move || {
        let mut was_inside = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(if was_inside {
                INSIDE_POLL_MS
            } else {
                OUTSIDE_POLL_MS
            }));

            let Some(overlay) = app.get_webview_window("overlay") else {
                continue;
            };
            let (Ok(cursor), Ok(position), Ok(size)) = (
                app.cursor_position(),
                overlay.outer_position(),
                overlay.outer_size(),
            ) else {
                continue;
            };

            let inside = cursor.x >= position.x as f64
                && cursor.x < position.x as f64 + size.width as f64
                && cursor.y >= position.y as f64
                && cursor.y < position.y as f64 + size.height as f64;

            // Emit every tick while inside (the webview hit-tests the
            // coordinates), plus one final event on exit.
            if inside || was_inside {
                let scale = overlay.scale_factor().unwrap_or(1.0);
                let _ = app.emit_to(
                    "overlay",
                    "overlay-cursor",
                    OverlayCursorPayload {
                        x: (cursor.x - position.x as f64) / scale,
                        y: (cursor.y - position.y as f64) / scale,
                        inside,
                    },
                );
            }

            was_inside = inside;
        }
    });
}

fn get_app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.typr.app")
}

#[cfg(target_os = "macos")]
const START_ON_LOGIN_LABEL: &str = "com.typr.app.login";

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
fn launch_agent_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory".to_string())?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", START_ON_LOGIN_LABEL)))
}

#[cfg(target_os = "macos")]
fn current_app_bundle_path(exe_path: &Path) -> Option<PathBuf> {
    exe_path
        .ancestors()
        .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("app"))
        .map(Path::to_path_buf)
}

#[cfg(target_os = "macos")]
fn start_on_login_program_arguments(exe_path: &Path) -> Vec<String> {
    if let Some(app_path) = current_app_bundle_path(exe_path) {
        vec![
            "/usr/bin/open".to_string(),
            "-n".to_string(),
            app_path.to_string_lossy().to_string(),
        ]
    } else {
        vec![exe_path.to_string_lossy().to_string()]
    }
}

#[cfg(target_os = "macos")]
fn launch_agent_plist(program_arguments: &[String]) -> String {
    let arguments = program_arguments
        .iter()
        .map(|argument| format!("    <string>{}</string>", xml_escape(argument)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{}</string>
  <key>ProgramArguments</key>
  <array>
{}
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#,
        START_ON_LOGIN_LABEL, arguments
    )
}

#[cfg(target_os = "macos")]
fn set_start_on_login(enabled: bool) -> Result<(), String> {
    let path = launch_agent_path()?;

    if enabled {
        let exe_path = env::current_exe().map_err(|e| e.to_string())?;
        let program_arguments = start_on_login_program_arguments(&exe_path);
        let plist = launch_agent_plist(&program_arguments);
        let parent = path
            .parent()
            .ok_or("Could not resolve LaunchAgents directory".to_string())?;

        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        fs::write(path, plist).map_err(|e| e.to_string())?;
        return Ok(());
    }

    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_start_on_login(enabled: bool) -> Result<(), String> {
    if enabled {
        Err("Start on login is currently supported on macOS only".to_string())
    } else {
        Ok(())
    }
}

fn overlay_frame(
    app: &tauri::AppHandle,
    width: f64,
    height: f64,
    position: &str,
    dock_inset: i32,
) -> (f64, f64) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let screen_position = monitor.position();
        let screen_size = monitor.size();
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let screen_x = screen_position.x as f64 / scale;
        let screen_y = screen_position.y as f64 / scale;
        let screen_w = screen_size.width as f64 / scale;
        let screen_h = screen_size.height as f64 / scale;
        let logical_x = work_area.position.x as f64 / scale;
        let logical_y = work_area.position.y as f64 / scale;
        let logical_w = work_area.size.width as f64 / scale;
        let logical_h = work_area.size.height as f64 / scale;
        let inset = dock_inset as f64;
        let edge_margin = OVERLAY_EDGE_MARGIN + inset;
        let dock_gap = OVERLAY_DOCK_GAP + inset;

        let x = match position {
            "bottom-left" | "top-left" => logical_x + edge_margin,
            "bottom-right" | "top-right" => logical_x + logical_w - width - edge_margin,
            _ => logical_x + ((logical_w - width) / 2.0),
        };
        let y = match position {
            "top-center" | "top-left" | "top-right" => logical_y + edge_margin,
            _ => logical_y + logical_h - height - dock_gap,
        };

        (
            clamp_axis_to_screen(x, screen_x, screen_w, width),
            clamp_axis_to_screen(y, screen_y, screen_h, height),
        )
    } else {
        (700.0, 720.0)
    }
}

fn clamp_axis_to_screen(
    value: f64,
    screen_start: f64,
    screen_length: f64,
    item_length: f64,
) -> f64 {
    let screen_end = screen_start + screen_length;
    let max_start = screen_end - item_length;

    if max_start <= screen_start {
        screen_start
    } else {
        value.clamp(screen_start, max_start)
    }
}

fn dock_size_scale(size: &str) -> f64 {
    match size {
        "compact" => 0.88,
        "large" => 1.16,
        _ => 1.0,
    }
}

fn min_overlay_preset_for_layout(layout: &str) -> OverlayWindowPreset {
    match layout {
        "parked" => OverlayWindowPreset {
            width: 82.0,
            height: 34.0,
        },
        "dock" => OverlayWindowPreset {
            width: 340.0,
            height: 62.0,
        },
        "label" => OverlayWindowPreset {
            width: 340.0,
            height: 98.0,
        },
        "menu" => OverlayWindowPreset {
            width: 340.0,
            height: 320.0,
        },
        "recording" => OverlayWindowPreset {
            width: 320.0,
            height: 70.0,
        },
        "status" => OverlayWindowPreset {
            width: 280.0,
            height: 58.0,
        },
        _ => OverlayWindowPreset {
            width: 280.0,
            height: 58.0,
        },
    }
}

fn overlay_preset_for_layout(
    layout: &str,
    dock_size: &str,
    dock_width_offset: i32,
    dock_height_offset: i32,
) -> Result<OverlayWindowPreset, String> {
    let preset = match layout {
        "parked" => Ok(OVERLAY_PARKED),
        "dock" => Ok(OVERLAY_DOCK),
        "label" => Ok(OVERLAY_LABEL),
        "menu" => Ok(OVERLAY_MENU),
        "recording" => Ok(OVERLAY_RECORDING),
        "status" => Ok(OVERLAY_STATUS),
        _ => Err(format!("Unknown overlay layout: {}", layout)),
    }?;
    let scale = dock_size_scale(dock_size);
    let min = min_overlay_preset_for_layout(layout);

    Ok(OverlayWindowPreset {
        width: ((preset.width * scale) + dock_width_offset as f64).max(min.width),
        height: ((preset.height * scale) + dock_height_offset as f64).max(min.height),
    })
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

fn is_modifier_token(token: &str) -> bool {
    matches!(
        token.trim().to_uppercase().as_str(),
        "OPTION"
            | "ALT"
            | "CONTROL"
            | "CTRL"
            | "COMMAND"
            | "CMD"
            | "SUPER"
            | "SHIFT"
            | "COMMANDORCONTROL"
            | "COMMANDORCTRL"
            | "CMDORCTRL"
            | "CMDORCONTROL"
    )
}

fn is_modifier_only_hotkey(hotkey: &str) -> bool {
    let tokens = hotkey
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    !tokens.is_empty() && tokens.iter().all(|token| is_modifier_token(token))
}

fn handle_hotkey_transition(handle: tauri::AppHandle, transition: ShortcutState) {
    let state = handle.state::<AppState>();
    let mode = state.settings.lock().unwrap().recording_mode.clone();
    println!("[Typr] Recording mode: {}", mode);

    match transition {
        ShortcutState::Pressed => {
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                match mode.as_str() {
                    "toggle" => {
                        println!("[Typr] Toggle mode: calling do_toggle_recording");
                        match do_toggle_recording(&handle, state.inner()).await {
                            Ok(result) => println!("[Typr] Toggle result: {}", result),
                            Err(e) => eprintln!("[Typr] Toggle error: {}", e),
                        }
                    }
                    "push-to-talk" => {
                        let current = state.recorder.get_state();
                        println!("[Typr] PTT mode, current state: {:?}", current);
                        if current == RecordingState::Ready {
                            let mic = state.settings.lock().unwrap().microphone.clone();
                            match state.recorder.start_recording(&handle, &mic) {
                                Ok(_) => println!("[Typr] Recording started"),
                                Err(e) => eprintln!("[Typr] Start recording error: {}", e),
                            }
                        }
                    }
                    _ => {}
                }
            });
        }
        ShortcutState::Released => {
            if mode == "push-to-talk" {
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<AppState>();
                    let current = state.recorder.get_state();
                    if current == RecordingState::Recording {
                        let settings = state.settings.lock().unwrap().clone();
                        match state
                            .recorder
                            .stop_and_transcribe(&handle, &settings, &state.app_dir)
                            .await
                        {
                            Ok(result) => println!("[Typr] Transcription: {}", result),
                            Err(e) => eprintln!("[Typr] Transcription error: {}", e),
                        }
                    }
                });
            }
        }
    }
}

fn register_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        println!("[Typr] Global shortcut disabled");
        return Ok(());
    }

    println!("[Typr] Registering global shortcut: {}", hotkey);
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey, move |_app, shortcut, event| {
            println!(
                "[Typr] Hotkey event: {:?} state={:?}",
                shortcut, event.state
            );
            handle_hotkey_transition(handle.clone(), event.state);
        })
        .map_err(|e| format!("Failed to register global shortcut '{}': {}", hotkey, e))?;

    println!("[Typr] Global shortcut registered successfully");
    Ok(())
}

fn stop_modifier_hotkey_watcher(state: &AppState) {
    if let Some(mut watcher) = state.modifier_hotkey_watcher.lock().unwrap().take() {
        watcher.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = watcher.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(target_os = "macos")]
fn start_modifier_hotkey_watcher(
    app: &tauri::AppHandle,
    state: &AppState,
    hotkey: &str,
) -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventFlags};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use std::time::Duration;

    fn modifier_mask_for_hotkey(hotkey: &str) -> Option<CGEventFlags> {
        let mut mask = CGEventFlags::empty();

        for token in hotkey
            .split('+')
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            match token.to_uppercase().as_str() {
                "OPTION" | "ALT" => mask |= CGEventFlags::CGEventFlagAlternate,
                "CONTROL" | "CTRL" => mask |= CGEventFlags::CGEventFlagControl,
                "COMMAND" | "CMD" | "SUPER" => mask |= CGEventFlags::CGEventFlagCommand,
                "SHIFT" => mask |= CGEventFlags::CGEventFlagShift,
                "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCTRL" | "CMDORCONTROL" => {
                    mask |= CGEventFlags::CGEventFlagCommand
                }
                _ => return None,
            }
        }

        if mask.is_empty() {
            None
        } else {
            Some(mask)
        }
    }

    fn current_modifier_flags() -> Option<CGEventFlags> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
        let event = CGEvent::new(source).ok()?;
        Some(event.get_flags())
    }

    let target_mask = modifier_mask_for_hotkey(hotkey)
        .ok_or_else(|| format!("Invalid modifier-only hotkey '{}'", hotkey))?;
    let modifier_mask = CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagAlternate
        | CGEventFlags::CGEventFlagCommand;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let handle = app.clone();
    let hotkey_name = hotkey.to_string();

    stop_modifier_hotkey_watcher(state);

    let thread = std::thread::spawn(move || {
        let mut was_pressed = current_modifier_flags()
            .map(|flags| (flags & modifier_mask) == target_mask)
            .unwrap_or(false);

        while !thread_stop.load(Ordering::Relaxed) {
            if let Some(flags) = current_modifier_flags() {
                let pressed = (flags & modifier_mask) == target_mask;

                if pressed != was_pressed {
                    println!(
                        "[Typr] Modifier-only hotkey '{}' {}",
                        hotkey_name,
                        if pressed { "pressed" } else { "released" }
                    );
                    handle_hotkey_transition(
                        handle.clone(),
                        if pressed {
                            ShortcutState::Pressed
                        } else {
                            ShortcutState::Released
                        },
                    );
                    was_pressed = pressed;
                }
            }

            std::thread::sleep(Duration::from_millis(25));
        }
    });

    *state.modifier_hotkey_watcher.lock().unwrap() = Some(ModifierHotkeyWatcher {
        stop,
        handle: Some(thread),
    });
    println!("[Typr] Modifier-only hotkey watcher registered: {}", hotkey);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn start_modifier_hotkey_watcher(
    _app: &tauri::AppHandle,
    _state: &AppState,
    hotkey: &str,
) -> Result<(), String> {
    Err(format!(
        "Modifier-only hotkeys are currently supported on macOS only: {}",
        hotkey
    ))
}

fn update_registered_hotkey(
    app: &tauri::AppHandle,
    state: &AppState,
    previous_hotkey: &str,
    next_hotkey: &str,
) -> Result<(), String> {
    let previous_hotkey = previous_hotkey.trim();
    let next_hotkey = next_hotkey.trim();

    if previous_hotkey == next_hotkey {
        return Ok(());
    }

    stop_modifier_hotkey_watcher(state);

    if !previous_hotkey.is_empty() && !is_modifier_only_hotkey(previous_hotkey) {
        if let Err(e) = app.global_shortcut().unregister(previous_hotkey) {
            eprintln!(
                "[Typr] Failed to unregister previous shortcut '{}': {}",
                previous_hotkey, e
            );
        }
    }

    let register_result = if next_hotkey.is_empty() {
        Ok(())
    } else if is_modifier_only_hotkey(next_hotkey) {
        start_modifier_hotkey_watcher(app, state, next_hotkey)
    } else {
        register_hotkey(app, next_hotkey)
    };

    if let Err(e) = register_result {
        if !previous_hotkey.is_empty() {
            if is_modifier_only_hotkey(previous_hotkey) {
                let _ = start_modifier_hotkey_watcher(app, state, previous_hotkey);
            } else {
                let _ = register_hotkey(app, previous_hotkey);
            }
        }
        return Err(e);
    }

    Ok(())
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    mut settings: Settings,
) -> Result<(), String> {
    settings.hotkey = settings.hotkey.trim().to_string();
    settings.normalize_model_preferences();
    settings.normalize_language_preferences();
    settings.normalize_dock_preferences();
    settings.normalize_transforms();
    let previous_hotkey = state.settings.lock().unwrap().hotkey.clone();
    update_registered_hotkey(&app, state.inner(), &previous_hotkey, &settings.hotkey)?;
    set_start_on_login(settings.start_on_login)?;

    settings.save(&state.app_dir)?;
    *state.settings.lock().unwrap() = settings.clone();
    let _ = app.emit("settings-updated", settings);
    Ok(())
}

#[tauri::command]
fn list_microphones() -> Vec<audio::MicDevice> {
    audio::list_microphones()
}

#[tauri::command]
fn get_recording_state(state: State<AppState>) -> RecordingState {
    state.recorder.get_state()
}

#[tauri::command]
fn get_history(state: State<AppState>) -> Vec<TranscriptHistoryEntry> {
    history::load_history(&state.app_dir)
}

#[tauri::command]
fn get_history_audio(state: State<AppState>, id: String) -> Result<Vec<u8>, String> {
    history::read_history_audio(&state.app_dir, &id)
}

#[tauri::command]
fn clear_history(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    history::clear_history(&state.app_dir)?;
    let _ = app.emit("history-updated", ());
    Ok(())
}

#[tauri::command]
fn check_model_downloaded(state: State<AppState>, model_size: String) -> bool {
    let model_file = transcribe_local::model_filename(&model_size);
    state.app_dir.join(&model_file).exists()
}

#[tauri::command]
fn set_overlay_layout(
    app: tauri::AppHandle,
    state: State<AppState>,
    layout: String,
) -> Result<(), String> {
    let overlay = app
        .get_webview_window("overlay")
        .ok_or("Overlay window not found".to_string())?;
    let settings = state.settings.lock().unwrap().clone();
    let preset = overlay_preset_for_layout(
        &layout,
        &settings.dock_size,
        settings.dock_width_offset,
        settings.dock_height_offset,
    )?;
    let width = preset.width;
    let height = preset.height;
    let (x, y) = overlay_frame(
        &app,
        width,
        height,
        &settings.dock_position,
        settings.dock_inset,
    );

    overlay
        .set_size(Size::Logical(LogicalSize { width, height }))
        .map_err(|e| e.to_string())?;
    overlay
        .set_position(Position::Logical(LogicalPosition { x, y }))
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn show_notes_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("notes") {
        window.show().map_err(|e| e.to_string())?;
        window.unminimize().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, "notes", WebviewUrl::App("src/notes.html".into()))
        .title("Typr Notes")
        .inner_size(NOTES_WIDTH, NOTES_HEIGHT)
        .min_inner_size(300.0, 320.0)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn hide_notes_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("notes") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn show_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Settings window not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    model_size: String,
) -> Result<(), String> {
    let url = transcribe_local::model_download_url(&model_size);
    let model_file = transcribe_local::model_filename(&model_size);
    let dest = state.app_dir.join(&model_file);
    downloader::download_model(app, &url, &dest).await
}

#[tauri::command]
async fn toggle_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    do_toggle_recording(&app, &state).await
}

#[tauri::command]
fn cancel_recording(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    state.recorder.cancel_recording(&app)
}

/// Shared logic for toggle recording, used by both the Tauri command and hotkey handler.
async fn do_toggle_recording(app: &tauri::AppHandle, state: &AppState) -> Result<String, String> {
    let current_state = state.recorder.get_state();
    match current_state {
        RecordingState::Ready => {
            let mic = state.settings.lock().unwrap().microphone.clone();
            state.recorder.start_recording(app, &mic)?;
            Ok("recording".to_string())
        }
        RecordingState::Recording => {
            let settings = state.settings.lock().unwrap().clone();
            let result = state
                .recorder
                .stop_and_transcribe(app, &settings, &state.app_dir)
                .await?;
            Ok(result)
        }
        RecordingState::Transcribing => Err("Currently transcribing, please wait".to_string()),
    }
}

fn main() {
    let app_dir = get_app_dir();
    let settings = Settings::load(&app_dir);
    if settings.start_on_login {
        if let Err(e) = set_start_on_login(true) {
            eprintln!("[Typr] Failed to refresh start on login item: {}", e);
        }
    }
    let app_dir_for_setup = app_dir.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            recorder: Recorder::new(),
            settings: Mutex::new(settings),
            app_dir,
            modifier_hotkey_watcher: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_microphones,
            get_recording_state,
            get_history,
            get_history_audio,
            clear_history,
            check_model_downloaded,
            set_overlay_layout,
            show_notes_window,
            hide_notes_window,
            show_settings_window,
            download_model,
            toggle_recording,
            cancel_recording,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let initial_settings = Settings::load(&app_dir_for_setup);
            let initial_preset = overlay_preset_for_layout(
                "parked",
                &initial_settings.dock_size,
                initial_settings.dock_width_offset,
                initial_settings.dock_height_offset,
            )
            .unwrap_or(OVERLAY_PARKED);
            let (x, y) = overlay_frame(
                &handle,
                initial_preset.width,
                initial_preset.height,
                &initial_settings.dock_position,
                initial_settings.dock_inset,
            );

            let overlay = WebviewWindowBuilder::new(
                app,
                "overlay",
                WebviewUrl::App("src/overlay.html".into()),
            )
            .title("")
            .inner_size(initial_preset.width, initial_preset.height)
            .position(x, y)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .accept_first_mouse(true)
            .shadow(false)
            .build();

            match overlay {
                Ok(_) => {
                    println!("[Typr] Overlay window created");
                    start_overlay_cursor_watcher(handle.clone());
                }
                Err(e) => eprintln!("[Typr] Failed to create overlay: {}", e),
            }

            let state = handle.state::<AppState>();
            let initial_hotkey = state.settings.lock().unwrap().hotkey.clone();
            if let Err(e) = update_registered_hotkey(&handle, state.inner(), "", &initial_hotkey) {
                eprintln!("[Typr] ERROR: {}", e);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_axis_to_screen_uses_physical_edge_as_limit() {
        assert_eq!(clamp_axis_to_screen(-24.0, 0.0, 1440.0, 120.0), 0.0);
        assert_eq!(clamp_axis_to_screen(1360.0, 0.0, 1440.0, 120.0), 1320.0);
        assert_eq!(clamp_axis_to_screen(48.0, 0.0, 1440.0, 120.0), 48.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launch_agent_plist_escapes_program_arguments() {
        let plist = launch_agent_plist(&[
            "/usr/bin/open".to_string(),
            "/Applications/Typr & More.app".to_string(),
        ]);

        assert!(plist.contains("<string>com.typr.app.login</string>"));
        assert!(plist.contains("<string>/Applications/Typr &amp; More.app</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn start_on_login_arguments_open_app_bundle_when_available() {
        let args = start_on_login_program_arguments(Path::new(
            "/Applications/Typr.app/Contents/MacOS/typr",
        ));

        assert_eq!(
            args,
            vec![
                "/usr/bin/open".to_string(),
                "-n".to_string(),
                "/Applications/Typr.app".to_string(),
            ]
        );
    }
}
