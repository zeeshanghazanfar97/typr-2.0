use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptTransformHistory {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub model: String,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: String,
    #[serde(rename = "inputText")]
    pub input_text: String,
    #[serde(rename = "outputText")]
    pub output_text: String,
    pub status: String,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptAudioHistory {
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptHistoryEntry {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    pub engine: String,
    pub microphone: String,
    #[serde(rename = "transcriptModel")]
    pub transcript_model: String,
    #[serde(rename = "whisperModel")]
    pub whisper_model: String,
    #[serde(rename = "transcriptLanguages", default)]
    pub transcript_languages: Vec<String>,
    #[serde(rename = "rawTranscript")]
    pub raw_transcript: String,
    #[serde(rename = "cleanedTranscript")]
    pub cleaned_transcript: String,
    #[serde(rename = "finalText")]
    pub final_text: String,
    pub transform: Option<TranscriptTransformHistory>,
    #[serde(rename = "pasteStatus")]
    pub paste_status: String,
    #[serde(default)]
    pub audio: Option<TranscriptAudioHistory>,
}

pub fn history_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("history.json")
}

pub fn history_audio_dir(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("history-audio")
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

impl TranscriptHistoryEntry {
    pub fn new(
        engine: String,
        microphone: String,
        transcript_model: String,
        whisper_model: String,
        transcript_languages: Vec<String>,
        raw_transcript: String,
        cleaned_transcript: String,
        final_text: String,
        transform: Option<TranscriptTransformHistory>,
        paste_status: String,
    ) -> Self {
        let created_at = now_millis();
        Self {
            id: format!("history-{}", created_at),
            created_at,
            engine,
            microphone,
            transcript_model,
            whisper_model,
            transcript_languages,
            raw_transcript,
            cleaned_transcript,
            final_text,
            transform,
            paste_status,
            audio: None,
        }
    }
}

fn audio_filename_for_entry(entry_id: &str) -> Result<String, String> {
    if entry_id.is_empty()
        || !entry_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("Invalid history entry id".to_string());
    }

    Ok(format!("{}.wav", entry_id))
}

fn audio_path_for_filename(app_dir: &PathBuf, file_name: &str) -> Result<PathBuf, String> {
    let file_path = Path::new(file_name);
    if file_name.is_empty()
        || file_path.components().count() != 1
        || file_path.file_name().and_then(|name| name.to_str()) != Some(file_name)
        || !file_name.ends_with(".wav")
    {
        return Err("Invalid history audio file".to_string());
    }

    Ok(history_audio_dir(app_dir).join(file_name))
}

pub fn store_history_audio(
    app_dir: &PathBuf,
    entry_id: &str,
    source_path: &PathBuf,
) -> Result<TranscriptAudioHistory, String> {
    let file_name = audio_filename_for_entry(entry_id)?;
    let audio_dir = history_audio_dir(app_dir);
    fs::create_dir_all(&audio_dir).map_err(|e| e.to_string())?;
    let destination = audio_dir.join(&file_name);

    fs::copy(source_path, &destination).map_err(|e| e.to_string())?;
    let size_bytes = fs::metadata(&destination).map_err(|e| e.to_string())?.len();

    Ok(TranscriptAudioHistory {
        file_name,
        mime_type: "audio/wav".to_string(),
        size_bytes,
    })
}

pub fn read_history_audio(app_dir: &PathBuf, entry_id: &str) -> Result<Vec<u8>, String> {
    let history = load_history(app_dir);
    let entry = history
        .iter()
        .find(|entry| entry.id == entry_id)
        .ok_or("History entry not found".to_string())?;
    let audio = entry
        .audio
        .as_ref()
        .ok_or("History entry has no audio recording".to_string())?;
    let path = audio_path_for_filename(app_dir, &audio.file_name)?;

    fs::read(path).map_err(|e| e.to_string())
}

pub fn load_history(app_dir: &PathBuf) -> Vec<TranscriptHistoryEntry> {
    let path = history_path(app_dir);
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut history: Vec<TranscriptHistoryEntry> =
        serde_json::from_str(&contents).unwrap_or_default();
    history.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    history
}

pub fn save_history(app_dir: &PathBuf, history: &[TranscriptHistoryEntry]) -> Result<(), String> {
    fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
    let path = history_path(app_dir);
    let json = serde_json::to_string_pretty(history).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn append_history(app_dir: &PathBuf, entry: TranscriptHistoryEntry) -> Result<(), String> {
    let mut history = load_history(app_dir);
    history.insert(0, entry);
    save_history(app_dir, &history)
}

pub fn clear_history(app_dir: &PathBuf) -> Result<(), String> {
    let path = history_path(app_dir);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    let audio_dir = history_audio_dir(app_dir);
    if audio_dir.exists() {
        fs::remove_dir_all(audio_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    fn sample_entry(created_at: u64) -> TranscriptHistoryEntry {
        TranscriptHistoryEntry {
            id: format!("history-{}", created_at),
            created_at,
            engine: "cloud".to_string(),
            microphone: "default".to_string(),
            transcript_model: "whisper-large-v3-turbo".to_string(),
            whisper_model: "small".to_string(),
            transcript_languages: vec!["en".to_string(), "ur".to_string()],
            raw_transcript: "raw words".to_string(),
            cleaned_transcript: "Raw words.".to_string(),
            final_text: "Final words.".to_string(),
            transform: Some(TranscriptTransformHistory {
                id: "polish".to_string(),
                name: "Polish".to_string(),
                icon: "sparkles".to_string(),
                model: "openai/gpt-oss-120b".to_string(),
                system_prompt: "Polish text.".to_string(),
                input_text: "Raw words.".to_string(),
                output_text: "Final words.".to_string(),
                status: "applied".to_string(),
                error: String::new(),
            }),
            paste_status: "pasted".to_string(),
            audio: None,
        }
    }

    #[test]
    fn test_append_and_load_history_newest_first() {
        let dir = temp_dir().join("typr_test_history");
        let _ = fs::remove_dir_all(&dir);

        append_history(&dir, sample_entry(100)).unwrap();
        append_history(&dir, sample_entry(200)).unwrap();

        let history = load_history(&dir);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].created_at, 200);
        assert_eq!(history[1].created_at, 100);
        assert_eq!(
            history[0].transform.as_ref().unwrap().output_text,
            "Final words."
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clear_history_removes_file() {
        let dir = temp_dir().join("typr_test_history_clear");
        let _ = fs::remove_dir_all(&dir);

        append_history(&dir, sample_entry(100)).unwrap();
        assert!(!load_history(&dir).is_empty());

        clear_history(&dir).unwrap();
        assert!(load_history(&dir).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_store_and_read_history_audio() {
        let dir = temp_dir().join("typr_test_history_audio");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let source = dir.join("source.wav");
        fs::write(&source, b"RIFFtypr").unwrap();

        let mut entry = sample_entry(300);
        entry.audio = Some(store_history_audio(&dir, &entry.id, &source).unwrap());
        append_history(&dir, entry.clone()).unwrap();

        assert_eq!(entry.audio.as_ref().unwrap().file_name, "history-300.wav");
        assert_eq!(read_history_audio(&dir, &entry.id).unwrap(), b"RIFFtypr");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clear_history_removes_audio_dir() {
        let dir = temp_dir().join("typr_test_history_audio_clear");
        let _ = fs::remove_dir_all(&dir);
        let audio_dir = history_audio_dir(&dir);
        fs::create_dir_all(&audio_dir).unwrap();
        fs::write(audio_dir.join("history-100.wav"), b"RIFF").unwrap();

        append_history(&dir, sample_entry(100)).unwrap();
        assert!(audio_dir.exists());

        clear_history(&dir).unwrap();
        assert!(!history_path(&dir).exists());
        assert!(!audio_dir.exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
