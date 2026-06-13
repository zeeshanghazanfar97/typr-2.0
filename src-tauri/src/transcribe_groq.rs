use reqwest::multipart;
use std::path::PathBuf;

use crate::settings::TextTransform;

pub async fn transcribe_groq(
    api_key: &str,
    audio_path: &PathBuf,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("Groq API key not set. Please enter your API key in settings.".to_string());
    }

    let audio_bytes =
        std::fs::read(audio_path).map_err(|e| format!("Failed to read audio file: {}", e))?;

    let file_part = multipart::Part::bytes(audio_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = multipart::Form::new()
        .text("model", model.to_string())
        .text("language", "en")
        .text("response_format", "json")
        .part("file", file_part);

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Groq API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Groq API error ({}): {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    json["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No 'text' field in Groq response".to_string())
}

fn transform_user_message(trimmed: &str) -> String {
    format!("Text to transform:\n{}\n\nTransformed text:", trimmed)
}

pub async fn transform_transcript_groq(
    api_key: &str,
    transcript: &str,
    transform: &TextTransform,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("Groq API key not set. Please enter your API key in settings.".to_string());
    }

    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let payload = serde_json::json!({
        "model": model,
        "temperature": 0.1,
        "max_tokens": 2048,
        "messages": [
            {
                "role": "system",
                "content": transform.system_prompt
            },
            {
                "role": "user",
                "content": transform_user_message(trimmed)
            }
        ]
    });

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Groq polish request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Groq polish error ({}): {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq polish response: {}", e))?;

    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or("No polished text in Groq response".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_api_key() {
        let path = PathBuf::from("/tmp/test.wav");
        let result = transcribe_groq("", &path, "whisper-large-v3-turbo").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key not set"));
    }

    #[tokio::test]
    async fn test_transform_empty_api_key() {
        let transform = TextTransform {
            id: "polish".to_string(),
            name: "Polish".to_string(),
            icon: "sparkles".to_string(),
            system_prompt: "Polish text.".to_string(),
        };
        let result = transform_transcript_groq(
            "",
            "I have to go there correction I want to go there",
            &transform,
            "openai/gpt-oss-120b",
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key not set"));
    }

    #[tokio::test]
    async fn test_transform_empty_transcript_short_circuits() {
        let transform = TextTransform {
            id: "promptEngineer".to_string(),
            name: "Prompt Engineer".to_string(),
            icon: "blocks".to_string(),
            system_prompt: "Create a prompt.".to_string(),
        };
        let result =
            transform_transcript_groq("test-key", "   ", &transform, "openai/gpt-oss-120b").await;
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_transform_user_message_wraps_input() {
        let message = transform_user_message("hi");
        assert!(message.starts_with("Text to transform:"));
        assert!(message.contains("Transformed text:"));
    }
}
