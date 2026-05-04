use crate::state::AppState;
use scribebuddy_core::audio::cpal_capture::CpalAudioSource;
use scribebuddy_core::audio::screencapturekit::ScreenCaptureKitSource;
use scribebuddy_core::transcription::model::ModelManager;
use scribebuddy_core::{
    export::markdown::MarkdownExporter, ModelSize, RunningApp, SessionConfig, SessionMeta,
    SessionState, TranscriptSegment,
};
use crossbeam_channel;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn list_running_apps() -> Vec<RunningApp> {
    ScreenCaptureKitSource::enumerate_running_apps()
}

#[tauri::command]
pub fn list_audio_devices() -> Vec<String> {
    CpalAudioSource::enumerate_input_devices()
}

#[tauri::command]
pub fn get_session_state(state: State<'_, AppState>) -> SessionState {
    state.session.lock().state()
}

#[tauri::command]
pub fn set_target_app(
    state: State<'_, AppState>,
    bundle_id: String,
) -> Result<(), String> {
    { state.config.lock().target_app_bundle_id = Some(bundle_id); }
    state.save_config()
}

#[tauri::command]
pub fn set_model_size(
    state: State<'_, AppState>,
    size: String,
) -> Result<(), String> {
    let model_size = match size.as_str() {
        "tiny" => ModelSize::Tiny,
        "base" => ModelSize::Base,
        "small" => ModelSize::Small,
        "medium" => ModelSize::Medium,
        "large" => ModelSize::Large,
        _ => return Err(format!("Unknown model size: {}", size)),
    };
    { state.config.lock().model_size = model_size; }
    state.save_config()
}

#[tauri::command]
pub fn set_language(
    state: State<'_, AppState>,
    language: String,
) -> Result<(), String> {
    { state.config.lock().language = language; }
    state.save_config()
}

#[tauri::command]
pub async fn set_chunk_duration(
    state: State<'_, AppState>,
    seconds: f32,
) -> Result<(), String> {
    let new_config = {
        let mut config = state.config.lock();
        config.chunk_duration_secs = seconds.clamp(1.0, 5.0);
        config.clone()
    };
    state.save_config()?;

    // If recording, transparently restart the processor with the new chunk size.
    let is_recording = state.session.lock().state() == SessionState::Recording;
    if is_recording {
        let segment_tx = state.segment_tx.lock().clone()
            .ok_or("Session channels not initialized")?;
        let error_tx = state.error_tx.lock().clone()
            .ok_or("Session channels not initialized")?;
        let mut session = state.session.lock();
        session.update_config(new_config);
        session.pause().map_err(|e| e.to_string())?;
        session.resume(segment_tx, error_tx).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_capture_mode(
    state: State<'_, AppState>,
    use_screencapturekit: bool,
) -> Result<(), String> {
    { state.config.lock().use_screencapturekit = use_screencapturekit; }
    state.save_config()
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<SessionConfig, String> {
    let config = state.config.lock();
    Ok(config.clone())
}

#[tauri::command]
pub fn check_model_available(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let config = state.config.lock().clone();
    let model_mgr = ModelManager::new().map_err(|e| e.to_string())?;
    Ok(model_mgr.is_model_available(&config.model_size, config.is_multilingual()))
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = state.config.lock().clone();
    let multilingual = config.is_multilingual();
    let model_mgr = ModelManager::new().map_err(|e| e.to_string())?;

    if model_mgr.is_model_available(&config.model_size, multilingual) {
        let _ = app.emit("model-download-progress", serde_json::json!({
            "downloaded": 100u64,
            "total": 100u64,
            "done": true,
        }));
        return Ok(());
    }

    *state.model_downloading.write() = true;
    let _ = app.emit("model-download-started", config.model_size);

    let app_clone = app.clone();
    let result = model_mgr.download_model(
        &config.model_size,
        multilingual,
        move |downloaded, total| {
            let _ = app_clone.emit("model-download-progress", serde_json::json!({
                "downloaded": downloaded,
                "total": total,
                "done": false,
            }));
        },
    );

    *state.model_downloading.write() = false;

    match result {
        Ok(()) => {
            let _ = app.emit("model-download-progress", serde_json::json!({
                "downloaded": 100u64,
                "total": 100u64,
                "done": true,
            }));
            Ok(())
        }
        Err(e) => {
            let _ = app.emit("model-download-error", e.to_string());
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn start_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = state.config.lock().clone();

    let model_mgr = ModelManager::new().map_err(|e| e.to_string())?;
    if !model_mgr.is_model_available(&config.model_size, config.is_multilingual()) {
        return Err("Whisper model not found. Download it first.".to_string());
    }

    state.clear_segments();

    let (segment_tx, segment_rx) = crossbeam_channel::bounded(512);
    let (error_tx, error_rx) = crossbeam_channel::bounded(16);

    // Store sender clones so listener threads outlive pause/resume cycles.
    *state.segment_tx.lock() = Some(segment_tx.clone());
    *state.error_tx.lock() = Some(error_tx.clone());

    {
        let mut session = state.session.lock();
        if config.use_screencapturekit {
            session.set_you_source(Box::new(CpalAudioSource::new("Microphone", 44100, 1)));
            session.set_remote_source(Box::new(ScreenCaptureKitSource::new(
                "System Audio",
                config.target_app_bundle_id.clone(),
            )));
        } else {
            session.set_you_source(Box::new(CpalAudioSource::new("Microphone", 44100, 1)));
            session.set_remote_source(Box::new(CpalAudioSource::new("BlackHole", 44100, 2)));
        }
        session.start(segment_tx.clone(), error_tx.clone()).map_err(|e| e.to_string())?;
    }

    let _ = app.emit("session-state-changed", SessionState::Recording);

    // Listener thread: drain segment channel, emit to frontend.
    // Runs until AppState drops its segment_tx clone (on stop).
    let acc = state.accumulated.clone();
    let app_clone = app.clone();
    std::thread::spawn(move || {
        while let Ok(segment) = segment_rx.recv() {
            let _ = app_clone.emit("transcript-segment", segment.clone());
            acc.write().push(segment);
        }
    });

    // Error listener: forward processor errors to frontend.
    let app_for_errors = app.clone();
    std::thread::spawn(move || {
        while let Ok(msg) = error_rx.recv() {
            let _ = app_for_errors.emit("session-error", msg);
        }
    });

    log::info!("Session started");
    Ok(())
}

#[tauri::command]
pub async fn pause_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.session.lock().pause().map_err(|e| e.to_string())?;
    let _ = app.emit("session-state-changed", SessionState::Paused);
    Ok(())
}

#[tauri::command]
pub async fn resume_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let segment_tx = state.segment_tx.lock().clone().ok_or("Session channels not initialized")?;
    let error_tx = state.error_tx.lock().clone().ok_or("Session channels not initialized")?;
    state.session.lock().resume(segment_tx, error_tx).map_err(|e| e.to_string())?;
    let _ = app.emit("session-state-changed", SessionState::Recording);
    Ok(())
}

#[tauri::command]
pub async fn stop_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.session.lock().stop().map_err(|e| e.to_string())?;

    // Drop stored senders → channels close → listener threads exit.
    *state.segment_tx.lock() = None;
    *state.error_tx.lock() = None;

    let _ = app.emit("session-state-changed", SessionState::Stopped);

    // Auto-save on a background thread so serializing a large transcript
    // doesn't block the Tauri command and freeze the UI.
    let segments = state.get_segments();
    let config_path = state.config_path.clone();
    std::thread::spawn(move || {
        if let Err(e) = crate::state::AppState::write_transcript(config_path, segments) {
            log::warn!("Auto-save transcript failed: {}", e);
        }
    });

    log::info!("Session stopped");
    Ok(())
}

#[tauri::command]
pub fn get_transcript(state: State<'_, AppState>) -> Vec<TranscriptSegment> {
    state.get_segments()
}

#[tauri::command]
pub fn export_markdown(state: State<'_, AppState>) -> Result<String, String> {
    let segments = state.get_segments();
    if segments.is_empty() {
        return Err("No transcript to export".to_string());
    }
    Ok(MarkdownExporter::export(&segments))
}

#[tauri::command]
pub async fn export_markdown_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let segments = state.get_segments();
    if segments.is_empty() {
        return Err("No transcript to export".to_string());
    }

    let content = MarkdownExporter::export(&segments);
    let default_name = format!(
        "meeting-{}.md",
        chrono::Local::now().format("%Y-%m-%d")
    );

    let file_path = app
        .dialog()
        .file()
        .set_title("Export Transcript")
        .set_file_name(&default_name)
        .add_filter("Markdown", &["md"])
        .blocking_save_file();

    match file_path {
        Some(tauri_plugin_dialog::FilePath::Path(path)) => {
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    state.list_sessions()
}

#[tauri::command]
pub fn load_session(
    state: State<'_, AppState>,
    timestamp: u64,
) -> Result<Vec<TranscriptSegment>, String> {
    state.load_session(timestamp)
}

#[tauri::command]
pub fn delete_session(
    state: State<'_, AppState>,
    timestamp: u64,
) -> Result<(), String> {
    state.delete_session(timestamp)
}

#[tauri::command]
pub async fn export_session_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
    timestamp: u64,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let segments = state.load_session(timestamp)?;
    if segments.is_empty() {
        return Err("Session has no segments to export".to_string());
    }

    let content = MarkdownExporter::export(&segments);
    let date = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| timestamp.to_string());
    let default_name = format!("meeting-{}.md", date);

    let file_path = app
        .dialog()
        .file()
        .set_title("Export Session")
        .set_file_name(&default_name)
        .add_filter("Markdown", &["md"])
        .blocking_save_file();

    match file_path {
        Some(tauri_plugin_dialog::FilePath::Path(path)) => {
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

