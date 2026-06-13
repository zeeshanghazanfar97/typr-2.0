use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

pub async fn transcribe_local(
    app: &AppHandle,
    model_path: &PathBuf,
    audio_path: &PathBuf,
    languages: &[String],
) -> Result<String, String> {
    if !model_path.exists() {
        return Err("Whisper model not found. Please download a model first.".to_string());
    }

    println!(
        "[Typr] Running whisper.cpp sidecar with model {:?}",
        model_path
    );

    let args = transcription_args(model_path, audio_path, languages);
    let output = app
        .shell()
        .sidecar("whisper-cpp")
        .map_err(|e| format!("Failed to create sidecar command: {}", e))?
        .args(args)
        .output()
        .await
        .map_err(|e| format!("Failed to run whisper.cpp: {}", e))?;

    if output.status.code() != Some(0) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("whisper.cpp failed: {}", stderr));
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    println!("[Typr] Whisper output: {}", text);
    Ok(text)
}

fn transcription_language_code(languages: &[String]) -> String {
    let codes = languages
        .iter()
        .map(|language| language.trim().to_lowercase())
        .filter(|language| !language.is_empty())
        .collect::<Vec<_>>();

    if codes.len() == 1 {
        codes[0].clone()
    } else if codes.len() > 1 {
        "auto".to_string()
    } else {
        "en".to_string()
    }
}

fn transcription_language_prompt(languages: &[String]) -> Option<String> {
    let codes = languages
        .iter()
        .map(|language| language.trim().to_lowercase())
        .filter(|language| !language.is_empty())
        .collect::<Vec<_>>();

    if codes.len() > 1 {
        Some(format!(
            "Possible spoken language codes: {}. Transcribe in the spoken language and preserve the original language.",
            codes.join(", ")
        ))
    } else {
        None
    }
}

fn transcription_args(
    model_path: &PathBuf,
    audio_path: &PathBuf,
    languages: &[String],
) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        model_path.to_string_lossy().to_string(),
        "-f".to_string(),
        audio_path.to_string_lossy().to_string(),
        "--no-timestamps".to_string(),
        "-l".to_string(),
        transcription_language_code(languages),
    ];

    if let Some(prompt) = transcription_language_prompt(languages) {
        args.push("--prompt".to_string());
        args.push(prompt);
    }

    args
}

pub fn model_filename(model_size: &str) -> String {
    format!("ggml-{}.bin", model_size)
}

pub fn model_download_url(model_size: &str) -> String {
    format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
        model_size
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_filename() {
        assert_eq!(model_filename("small"), "ggml-small.bin");
        assert_eq!(model_filename("medium"), "ggml-medium.bin");
    }

    #[test]
    fn test_model_download_url() {
        assert_eq!(
            model_download_url("small"),
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
        );
    }

    #[test]
    fn test_transcription_language_code_single_language() {
        assert_eq!(transcription_language_code(&["ur".to_string()]), "ur");
    }

    #[test]
    fn test_transcription_args_use_auto_and_prompt_for_multiple_languages() {
        let args = transcription_args(
            &PathBuf::from("/models/ggml-small.bin"),
            &PathBuf::from("/audio.wav"),
            &["en".to_string(), "ur".to_string()],
        );

        assert!(args.windows(2).any(|pair| pair == ["-l", "auto"]));
        assert!(args.iter().any(|arg| arg.contains("en, ur")));
    }
}
