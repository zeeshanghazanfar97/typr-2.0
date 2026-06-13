use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, RecvTimeoutError},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::AudioRecorder;
use crate::cleanup::cleanup_text;
use crate::history::{
    append_history, store_history_audio, TranscriptHistoryEntry, TranscriptTransformHistory,
};
use crate::paste::paste_text;
use crate::settings::Settings;
use crate::transcribe_groq;
use crate::transcribe_local;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum RecordingState {
    Ready,
    Recording,
    Transcribing,
}

#[derive(Clone, serde::Serialize)]
struct AudioLevelPayload {
    level: f32,
}

struct AudioLevelWatcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

fn update_overlay(app: &AppHandle, state: &RecordingState) {
    let state_name = match state {
        RecordingState::Ready => "ready",
        RecordingState::Recording => "recording",
        RecordingState::Transcribing => "transcribing",
    };
    update_overlay_state(app, state_name);
}

fn update_overlay_state(app: &AppHandle, state_name: &str) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let js = format!(
            "window.setTyprState && window.setTyprState('{}');",
            state_name
        );
        let _ = overlay.eval(&js);
    }
}

pub struct Recorder {
    state: Arc<Mutex<RecordingState>>,
    audio_recorder: Arc<Mutex<AudioRecorder>>,
    audio_level_watcher: Arc<Mutex<Option<AudioLevelWatcher>>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState::Ready)),
            audio_recorder: Arc::new(Mutex::new(AudioRecorder::new())),
            audio_level_watcher: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_state(&self) -> RecordingState {
        self.state.lock().unwrap().clone()
    }

    fn start_audio_level_watcher(&self, app: AppHandle, receiver: Receiver<f32>) {
        self.stop_audio_level_watcher();

        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(level) => {
                        let _ = app.emit_to(
                            "overlay",
                            "audio-level",
                            AudioLevelPayload {
                                level: level.clamp(0.0, 1.0),
                            },
                        );
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        *self.audio_level_watcher.lock().unwrap() = Some(AudioLevelWatcher {
            stop,
            handle: Some(handle),
        });
    }

    fn stop_audio_level_watcher(&self) {
        if let Some(mut watcher) = self.audio_level_watcher.lock().unwrap().take() {
            watcher.stop.store(true, Ordering::Relaxed);
            if let Some(handle) = watcher.handle.take() {
                let _ = handle.join();
            }
        }
    }

    pub fn start_recording(&self, app: &AppHandle, mic_name: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if *state != RecordingState::Ready {
            return Err("Already recording or transcribing".to_string());
        }

        let (level_sender, level_receiver) = mpsc::channel();
        let mut recorder = self.audio_recorder.lock().unwrap();
        recorder.start(mic_name, Some(level_sender))?;
        drop(recorder);
        self.start_audio_level_watcher(app.clone(), level_receiver);

        *state = RecordingState::Recording;
        let _ = app.emit("recording-state", RecordingState::Recording);
        update_overlay(app, &RecordingState::Recording);
        Ok(())
    }

    pub fn cancel_recording(&self, app: &AppHandle) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if *state != RecordingState::Recording {
            return Err("Not currently recording".to_string());
        }

        {
            let mut recorder = self.audio_recorder.lock().unwrap();
            recorder.cancel();
        }
        self.stop_audio_level_watcher();

        *state = RecordingState::Ready;
        let _ = app.emit("recording-state", RecordingState::Ready);
        update_overlay(app, &RecordingState::Ready);
        Ok(())
    }

    pub async fn stop_and_transcribe(
        &self,
        app: &AppHandle,
        settings: &Settings,
        app_dir: &PathBuf,
    ) -> Result<String, String> {
        // Stop recording
        {
            let mut state = self.state.lock().unwrap();
            if *state != RecordingState::Recording {
                return Err("Not currently recording".to_string());
            }
            *state = RecordingState::Transcribing;
            let _ = app.emit("recording-state", RecordingState::Transcribing);
            update_overlay(app, &RecordingState::Transcribing);
        }

        let temp_path = app_dir.join("temp_recording.wav");

        let result = async {
            // Save audio
            let save_result = {
                let mut recorder = self.audio_recorder.lock().unwrap();
                recorder.stop_and_save(&temp_path)
            };
            self.stop_audio_level_watcher();
            save_result?;

            // Transcribe
            let raw_text = match settings.engine.as_str() {
                "local" => {
                    let model_path =
                        app_dir.join(transcribe_local::model_filename(&settings.whisper_model));
                    transcribe_local::transcribe_local(app, &model_path, &temp_path).await?
                }
                "cloud" => {
                    transcribe_groq::transcribe_groq(
                        &settings.groq_api_key,
                        &temp_path,
                        settings.transcript_model(),
                    )
                    .await?
                }
                _ => return Err(format!("Unknown engine: {}", settings.engine)),
            };

            let cleaned_raw = cleanup_text(&raw_text);
            let mut final_text = cleaned_raw.clone();
            let mut transform_history = None;

            if settings.auto_polish && !cleaned_raw.is_empty() {
                update_overlay_state(app, "polishing");
                if let Some(transform) = settings.selected_transform() {
                    let transform_model = settings.transform_model().to_string();
                    match transcribe_groq::transform_transcript_groq(
                        &settings.groq_api_key,
                        &cleaned_raw,
                        transform,
                        &transform_model,
                    )
                    .await
                    {
                        Ok(polished) => {
                            let polished = cleanup_text(&polished);
                            let output_text = if polished.is_empty() {
                                cleaned_raw.clone()
                            } else {
                                polished
                            };
                            transform_history = Some(TranscriptTransformHistory {
                                id: transform.id.clone(),
                                name: transform.name.clone(),
                                icon: transform.icon.clone(),
                                model: transform_model,
                                system_prompt: transform.system_prompt.clone(),
                                input_text: cleaned_raw.clone(),
                                output_text: output_text.clone(),
                                status: "applied".to_string(),
                                error: String::new(),
                            });
                            final_text = output_text;
                        }
                        Err(e) => {
                            eprintln!("[Typr] Groq transform failed, using raw transcript: {}", e);
                            transform_history = Some(TranscriptTransformHistory {
                                id: transform.id.clone(),
                                name: transform.name.clone(),
                                icon: transform.icon.clone(),
                                model: transform_model,
                                system_prompt: transform.system_prompt.clone(),
                                input_text: cleaned_raw.clone(),
                                output_text: String::new(),
                                status: "failed".to_string(),
                                error: e,
                            });
                        }
                    }
                }
            }

            // Auto-paste
            let mut paste_status = if final_text.is_empty() {
                "skipped".to_string()
            } else {
                "pasted".to_string()
            };
            let paste_result = if final_text.is_empty() {
                Ok(())
            } else {
                paste_text(&final_text)
            };
            if paste_result.is_err() {
                paste_status = "failed".to_string();
            }

            let mut history_entry = TranscriptHistoryEntry::new(
                settings.engine.clone(),
                settings.microphone.clone(),
                settings.transcript_model().to_string(),
                settings.whisper_model.clone(),
                raw_text,
                cleaned_raw,
                final_text.clone(),
                transform_history,
                paste_status,
            );
            match store_history_audio(app_dir, &history_entry.id, &temp_path) {
                Ok(audio) => {
                    history_entry.audio = Some(audio);
                }
                Err(e) => {
                    eprintln!("[Typr] Failed to save recording history: {}", e);
                }
            }

            if let Err(e) = append_history(app_dir, history_entry) {
                eprintln!("[Typr] Failed to save transcript history: {}", e);
            } else {
                let _ = app.emit("history-updated", ());
            }

            paste_result?;

            Ok(final_text)
        }
        .await;

        self.stop_audio_level_watcher();
        let _ = std::fs::remove_file(&temp_path);

        // Reset state
        {
            let mut state = self.state.lock().unwrap();
            *state = RecordingState::Ready;
            let _ = app.emit("recording-state", RecordingState::Ready);
            update_overlay(app, &RecordingState::Ready);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_ready() {
        let recorder = Recorder::new();
        assert_eq!(recorder.get_state(), RecordingState::Ready);
    }
}
