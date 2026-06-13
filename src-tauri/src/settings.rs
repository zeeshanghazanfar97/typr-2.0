use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_HOTKEY: &str = "Ctrl+Option+Space";
const LEGACY_DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+Space";

fn default_auto_polish() -> bool {
    true
}

fn default_start_on_login() -> bool {
    false
}

fn default_transform() -> String {
    "polish".to_string()
}

fn default_dock_size() -> String {
    "regular".to_string()
}

fn default_dock_shape() -> String {
    "pill".to_string()
}

fn default_dock_color() -> String {
    "charcoal".to_string()
}

fn default_dock_position() -> String {
    "bottom-center".to_string()
}

fn default_dock_inset() -> i32 {
    0
}

fn default_dock_width_offset() -> i32 {
    0
}

fn default_dock_height_offset() -> i32 {
    0
}

fn default_model_mode() -> String {
    "default".to_string()
}

fn default_transcript_languages() -> Vec<String> {
    vec!["en".to_string()]
}

const SUPPORTED_TRANSCRIPT_LANGUAGE_CODES: &[&str] = &[
    "af", "am", "ar", "as", "az", "ba", "be", "bg", "bn", "bo", "br", "bs", "ca", "cs", "cy", "da",
    "de", "el", "en", "es", "et", "eu", "fa", "fi", "fo", "fr", "gl", "gu", "ha", "haw", "he",
    "hi", "hr", "ht", "hu", "hy", "id", "is", "it", "ja", "jw", "ka", "kk", "km", "kn", "ko", "la",
    "lb", "ln", "lo", "lt", "lv", "mg", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "ne", "nl",
    "nn", "no", "oc", "pa", "pl", "ps", "pt", "ro", "ru", "sa", "sd", "si", "sk", "sl", "sn", "so",
    "sq", "sr", "su", "sv", "sw", "ta", "te", "tg", "th", "tk", "tl", "tr", "tt", "uk", "ur", "uz",
    "vi", "yi", "yo", "zh", "yue",
];

const LEGACY_POLISH_PROMPT: &str = "You polish dictated speech-to-text transcripts. Return only the final corrected text. Preserve the speaker's meaning and do not add new facts. Apply explicit self-corrections, for example when the speaker says correction, I mean, rather, or sorry, keep the corrected wording and remove the abandoned wording. Remove filler words and accidental repetitions only when they are not meaningful. Restore punctuation and capitalization.";

const LEGACY_PROMPT_ENGINEER_PROMPT: &str = "You turn dictated speech into a clear, well-structured prompt for an AI assistant. Return only the final prompt text with no commentary. Preserve the speaker's intent, constraints, and details; do not invent requirements. Apply explicit self-corrections, for example when the speaker says correction, I mean, rather, or sorry, keep the corrected wording and remove the abandoned wording. Remove filler words and false starts. State the goal first, then context, then specific requirements as a short list when it helps, then the desired output format if one was mentioned. Restore punctuation and capitalization.";

const POLISH_PROMPT: &str = "Rewrite the selected text to make it clearer, more concise, and easier to read. Remove filler words, repetition, awkward phrasing, and unnecessary complexity. Reorder sentences if needed for readability. Add light structure only when it improves clarity. Preserve the original meaning, intent, technical terms, URLs, markdown, and the writer's natural tone. Do not make it sound overly formal or AI-generated. Return only the rewritten text.";

const PROMPT_ENGINEER_PROMPT: &str = r#"Transform the selected text into a clear, high-quality prompt for an AI assistant.

Structure the output with these sections:

Title:
A short, descriptive title for the prompt.

Role & stance:
Define what role the AI should take and how it should behave.

Task:
Clearly state what the AI needs to do.

Context:
Include all relevant background details, constraints, examples, and assumptions from the original text.

Output format:
Specify the desired structure, tone, length, and formatting of the answer.

Keep the user's original intent intact. Do not add unsupported requirements. Make the prompt specific, actionable, and easy for an AI model to follow. Return only the improved prompt."#;

pub const DEFAULT_TRANSCRIPT_MODEL_ID: &str = "whisper-large-v3-turbo";
pub const DEFAULT_TRANSFORM_MODEL_ID: &str = "openai/gpt-oss-120b";

fn default_transforms() -> Vec<TextTransform> {
    vec![
        TextTransform {
            id: "polish".to_string(),
            name: "Polish".to_string(),
            icon: "sparkles".to_string(),
            system_prompt: POLISH_PROMPT.to_string(),
        },
        TextTransform {
            id: "promptEngineer".to_string(),
            name: "Prompt Engineer".to_string(),
            icon: "blocks".to_string(),
            system_prompt: PROMPT_ENGINEER_PROMPT.to_string(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextTransform {
    pub id: String,
    pub name: String,
    pub icon: String,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub microphone: String,
    pub engine: String,
    #[serde(rename = "whisperModel")]
    pub whisper_model: String,
    #[serde(rename = "groqApiKey")]
    pub groq_api_key: String,
    #[serde(rename = "transcriptModelMode", default = "default_model_mode")]
    pub transcript_model_mode: String,
    #[serde(rename = "transcriptModelId", default)]
    pub transcript_model_id: String,
    #[serde(
        rename = "transcriptLanguages",
        default = "default_transcript_languages"
    )]
    pub transcript_languages: Vec<String>,
    #[serde(rename = "transformModelMode", default = "default_model_mode")]
    pub transform_model_mode: String,
    #[serde(rename = "transformModelId", default)]
    pub transform_model_id: String,
    #[serde(rename = "autoPolish", default = "default_auto_polish")]
    pub auto_polish: bool,
    #[serde(rename = "defaultTransform", default = "default_transform")]
    pub default_transform: String,
    #[serde(default = "default_transforms")]
    pub transforms: Vec<TextTransform>,
    #[serde(rename = "recordingMode")]
    pub recording_mode: String,
    #[serde(rename = "startOnLogin", default = "default_start_on_login")]
    pub start_on_login: bool,
    #[serde(rename = "dockSize", default = "default_dock_size")]
    pub dock_size: String,
    #[serde(rename = "dockShape", default = "default_dock_shape")]
    pub dock_shape: String,
    #[serde(rename = "dockColor", default = "default_dock_color")]
    pub dock_color: String,
    #[serde(rename = "dockPosition", default = "default_dock_position")]
    pub dock_position: String,
    #[serde(rename = "dockInset", default = "default_dock_inset")]
    pub dock_inset: i32,
    #[serde(rename = "dockWidthOffset", default = "default_dock_width_offset")]
    pub dock_width_offset: i32,
    #[serde(rename = "dockHeightOffset", default = "default_dock_height_offset")]
    pub dock_height_offset: i32,
    pub hotkey: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            microphone: "default".to_string(),
            engine: "local".to_string(),
            whisper_model: "small".to_string(),
            groq_api_key: String::new(),
            transcript_model_mode: default_model_mode(),
            transcript_model_id: String::new(),
            transcript_languages: default_transcript_languages(),
            transform_model_mode: default_model_mode(),
            transform_model_id: String::new(),
            auto_polish: true,
            default_transform: default_transform(),
            transforms: default_transforms(),
            recording_mode: "toggle".to_string(),
            start_on_login: false,
            dock_size: default_dock_size(),
            dock_shape: default_dock_shape(),
            dock_color: default_dock_color(),
            dock_position: default_dock_position(),
            dock_inset: default_dock_inset(),
            dock_width_offset: default_dock_width_offset(),
            dock_height_offset: default_dock_height_offset(),
            hotkey: DEFAULT_HOTKEY.to_string(),
        }
    }
}

impl Settings {
    pub fn config_path(app_dir: &PathBuf) -> PathBuf {
        app_dir.join("config.json")
    }

    pub fn load(app_dir: &PathBuf) -> Self {
        let path = Self::config_path(app_dir);
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Self>(&contents) {
                Ok(mut settings) => {
                    if settings.hotkey == LEGACY_DEFAULT_HOTKEY {
                        settings.hotkey = DEFAULT_HOTKEY.to_string();
                    }
                    settings.normalize_model_preferences();
                    settings.normalize_language_preferences();
                    settings.normalize_dock_preferences();
                    settings.normalize_transforms();
                    settings
                }
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, app_dir: &PathBuf) -> Result<(), String> {
        let path = Self::config_path(app_dir);
        fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
        let mut settings = self.clone();
        settings.normalize_model_preferences();
        settings.normalize_language_preferences();
        settings.normalize_dock_preferences();
        settings.normalize_transforms();
        let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())
    }

    pub fn selected_transform(&self) -> Option<&TextTransform> {
        self.transforms
            .iter()
            .find(|transform| transform.id == self.default_transform)
            .or_else(|| self.transforms.first())
    }

    pub fn transcript_model(&self) -> &str {
        if self.transcript_model_mode == "custom" && !self.transcript_model_id.trim().is_empty() {
            self.transcript_model_id.trim()
        } else {
            DEFAULT_TRANSCRIPT_MODEL_ID
        }
    }

    pub fn transform_model(&self) -> &str {
        if self.transform_model_mode == "custom" && !self.transform_model_id.trim().is_empty() {
            self.transform_model_id.trim()
        } else {
            DEFAULT_TRANSFORM_MODEL_ID
        }
    }

    pub fn normalize_language_preferences(&mut self) {
        let mut normalized = Vec::new();

        for language in &self.transcript_languages {
            let code = language.trim().to_lowercase();
            if SUPPORTED_TRANSCRIPT_LANGUAGE_CODES.contains(&code.as_str())
                && !normalized.contains(&code)
            {
                normalized.push(code);
            }
        }

        if normalized.is_empty() {
            normalized = default_transcript_languages();
        }

        self.transcript_languages = normalized;
    }

    pub fn normalize_model_preferences(&mut self) {
        if !matches!(self.transcript_model_mode.as_str(), "default" | "custom") {
            self.transcript_model_mode = default_model_mode();
        }

        if !matches!(self.transform_model_mode.as_str(), "default" | "custom") {
            self.transform_model_mode = default_model_mode();
        }

        self.transcript_model_id = self.transcript_model_id.trim().to_string();
        self.transform_model_id = self.transform_model_id.trim().to_string();
    }

    fn normalized_system_prompt(id: &str, system_prompt: &str, fallback: &TextTransform) -> String {
        let system_prompt = system_prompt.trim();

        if system_prompt.is_empty() {
            fallback.system_prompt.clone()
        } else if id == "polish" && system_prompt == LEGACY_POLISH_PROMPT {
            POLISH_PROMPT.to_string()
        } else if id == "promptEngineer" && system_prompt == LEGACY_PROMPT_ENGINEER_PROMPT {
            PROMPT_ENGINEER_PROMPT.to_string()
        } else {
            system_prompt.to_string()
        }
    }

    pub fn normalize_transforms(&mut self) {
        let defaults = default_transforms();
        let mut normalized = Vec::new();

        for (index, transform) in self.transforms.iter().enumerate() {
            let fallback = defaults
                .get(index)
                .or_else(|| defaults.first())
                .expect("default transforms are present");
            let id = transform.id.trim();

            if id.is_empty() {
                continue;
            }

            normalized.push(TextTransform {
                id: id.to_string(),
                name: if transform.name.trim().is_empty() {
                    fallback.name.clone()
                } else {
                    transform.name.trim().to_string()
                },
                icon: if transform.icon.trim().is_empty() {
                    fallback.icon.clone()
                } else {
                    transform.icon.trim().to_string()
                },
                system_prompt: Self::normalized_system_prompt(
                    id,
                    &transform.system_prompt,
                    fallback,
                ),
            });
        }

        if normalized.is_empty() {
            normalized = defaults;
        }

        if !normalized
            .iter()
            .any(|transform| transform.id == self.default_transform)
        {
            self.default_transform = normalized[0].id.clone();
        }

        self.transforms = normalized;
    }

    pub fn normalize_dock_preferences(&mut self) {
        if !matches!(self.dock_size.as_str(), "compact" | "regular" | "large") {
            self.dock_size = default_dock_size();
        }

        if !matches!(self.dock_shape.as_str(), "pill" | "rounded" | "square") {
            self.dock_shape = default_dock_shape();
        }

        if !matches!(
            self.dock_color.as_str(),
            "charcoal" | "graphite" | "plum" | "moss" | "rose"
        ) {
            self.dock_color = default_dock_color();
        }

        if !matches!(
            self.dock_position.as_str(),
            "bottom-center"
                | "bottom-left"
                | "bottom-right"
                | "top-center"
                | "top-left"
                | "top-right"
        ) {
            self.dock_position = default_dock_position();
        }

        self.dock_inset = self.dock_inset.clamp(-80, 80);
        self.dock_inset = ((self.dock_inset as f64 / 10.0).round() as i32) * 10;
        self.dock_width_offset = self.dock_width_offset.clamp(-20, 80);
        self.dock_width_offset = ((self.dock_width_offset as f64 / 10.0).round() as i32) * 10;
        self.dock_height_offset = self.dock_height_offset.clamp(-12, 24);
        self.dock_height_offset = ((self.dock_height_offset as f64 / 4.0).round() as i32) * 4;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.microphone, "default");
        assert_eq!(settings.engine, "local");
        assert_eq!(settings.whisper_model, "small");
        assert_eq!(settings.groq_api_key, "");
        assert_eq!(settings.transcript_model_mode, "default");
        assert_eq!(settings.transcript_model_id, "");
        assert_eq!(settings.transcript_model(), DEFAULT_TRANSCRIPT_MODEL_ID);
        assert_eq!(settings.transcript_languages, vec!["en"]);
        assert_eq!(settings.transform_model_mode, "default");
        assert_eq!(settings.transform_model_id, "");
        assert_eq!(settings.transform_model(), DEFAULT_TRANSFORM_MODEL_ID);
        assert!(settings.auto_polish);
        assert_eq!(settings.default_transform, "polish");
        assert_eq!(settings.transforms.len(), 2);
        assert_eq!(settings.transforms[0].name, "Polish");
        assert!(settings.transforms[0]
            .system_prompt
            .starts_with("Rewrite the selected text"));
        assert_eq!(settings.transforms[1].name, "Prompt Engineer");
        assert!(settings.transforms[1]
            .system_prompt
            .contains("Role & stance:"));
        assert_eq!(settings.recording_mode, "toggle");
        assert!(!settings.start_on_login);
        assert_eq!(settings.dock_size, "regular");
        assert_eq!(settings.dock_shape, "pill");
        assert_eq!(settings.dock_color, "charcoal");
        assert_eq!(settings.dock_position, "bottom-center");
        assert_eq!(settings.dock_inset, 0);
        assert_eq!(settings.dock_width_offset, 0);
        assert_eq!(settings.dock_height_offset, 0);
        assert_eq!(settings.hotkey, "Ctrl+Option+Space");
    }

    #[test]
    fn test_save_and_load() {
        let dir = temp_dir().join("typr_test_settings");
        let _ = fs::remove_dir_all(&dir);

        let mut settings = Settings::default();
        settings.engine = "cloud".to_string();
        settings.groq_api_key = "test-key-123".to_string();
        settings.transcript_model_mode = "custom".to_string();
        settings.transcript_model_id = "whisper-large-v3".to_string();
        settings.transcript_languages = vec!["ur".to_string(), "en".to_string()];
        settings.transform_model_mode = "custom".to_string();
        settings.transform_model_id = "llama-3.3-70b-versatile".to_string();
        settings.start_on_login = true;
        settings.dock_size = "large".to_string();
        settings.dock_shape = "rounded".to_string();
        settings.dock_color = "moss".to_string();
        settings.dock_position = "top-right".to_string();
        settings.dock_inset = 30;
        settings.dock_width_offset = 40;
        settings.dock_height_offset = 12;

        settings.save(&dir).unwrap();
        let loaded = Settings::load(&dir);

        assert_eq!(loaded.engine, "cloud");
        assert_eq!(loaded.groq_api_key, "test-key-123");
        assert_eq!(loaded.transcript_model_mode, "custom");
        assert_eq!(loaded.transcript_model_id, "whisper-large-v3");
        assert_eq!(loaded.transcript_model(), "whisper-large-v3");
        assert_eq!(loaded.transcript_languages, vec!["ur", "en"]);
        assert_eq!(loaded.transform_model_mode, "custom");
        assert_eq!(loaded.transform_model_id, "llama-3.3-70b-versatile");
        assert_eq!(loaded.transform_model(), "llama-3.3-70b-versatile");
        assert!(loaded.start_on_login);
        assert!(loaded.auto_polish);
        assert_eq!(loaded.transforms.len(), 2);
        assert_eq!(loaded.dock_size, "large");
        assert_eq!(loaded.dock_shape, "rounded");
        assert_eq!(loaded.dock_color, "moss");
        assert_eq!(loaded.dock_position, "top-right");
        assert_eq!(loaded.dock_inset, 30);
        assert_eq!(loaded.dock_width_offset, 40);
        assert_eq!(loaded.dock_height_offset, 12);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_legacy_settings_defaults_auto_polish() {
        let dir = temp_dir().join("typr_test_legacy_auto_polish");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("config.json"),
            r#"{
              "microphone": "default",
              "engine": "local",
              "whisperModel": "small",
              "groqApiKey": "",
              "recordingMode": "toggle",
              "hotkey": "Ctrl+Option+Space"
            }"#,
        )
        .unwrap();

        let loaded = Settings::load(&dir);
        assert!(loaded.auto_polish);
        assert_eq!(loaded.default_transform, "polish");
        assert_eq!(loaded.transforms.len(), 2);
        assert_eq!(loaded.transcript_model_mode, "default");
        assert_eq!(loaded.transcript_model_id, "");
        assert_eq!(loaded.transcript_languages, vec!["en"]);
        assert_eq!(loaded.transform_model_mode, "default");
        assert_eq!(loaded.transform_model_id, "");
        assert!(!loaded.start_on_login);
        assert_eq!(loaded.dock_size, "regular");
        assert_eq!(loaded.dock_shape, "pill");
        assert_eq!(loaded.dock_color, "charcoal");
        assert_eq!(loaded.dock_position, "bottom-center");
        assert_eq!(loaded.dock_inset, 0);
        assert_eq!(loaded.dock_width_offset, 0);
        assert_eq!(loaded.dock_height_offset, 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let dir = temp_dir().join("typr_test_missing");
        let _ = fs::remove_dir_all(&dir);
        let settings = Settings::load(&dir);
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn test_load_corrupt_json_returns_default() {
        let dir = temp_dir().join("typr_test_corrupt");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.json"), "not json").unwrap();

        let settings = Settings::load(&dir);
        assert_eq!(settings, Settings::default());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_migrates_legacy_default_hotkey() {
        let dir = temp_dir().join("typr_test_legacy_hotkey");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut settings = Settings::default();
        settings.hotkey = LEGACY_DEFAULT_HOTKEY.to_string();
        fs::write(
            dir.join("config.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let loaded = Settings::load(&dir);
        assert_eq!(loaded.hotkey, DEFAULT_HOTKEY);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_and_load_custom_transforms() {
        let dir = temp_dir().join("typr_test_custom_transforms");
        let _ = fs::remove_dir_all(&dir);

        let mut settings = Settings::default();
        settings.default_transform = "summarize".to_string();
        settings.transforms = vec![TextTransform {
            id: "summarize".to_string(),
            name: "Summarize".to_string(),
            icon: "list".to_string(),
            system_prompt: "Summarize the dictated text.".to_string(),
        }];

        settings.save(&dir).unwrap();
        let loaded = Settings::load(&dir);

        assert_eq!(loaded.default_transform, "summarize");
        assert_eq!(loaded.transforms.len(), 1);
        assert_eq!(
            loaded.transforms[0].system_prompt,
            "Summarize the dictated text."
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_normalize_transforms_repairs_missing_default() {
        let mut settings = Settings::default();
        settings.default_transform = "missing".to_string();
        settings.transforms = vec![TextTransform {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            icon: "pen".to_string(),
            system_prompt: "Rewrite clearly.".to_string(),
        }];

        settings.normalize_transforms();

        assert_eq!(settings.default_transform, "custom");
        assert_eq!(settings.selected_transform().unwrap().name, "Custom");
    }

    #[test]
    fn test_normalize_transforms_migrates_legacy_default_prompts() {
        let mut settings = Settings::default();
        settings.transforms = vec![
            TextTransform {
                id: "polish".to_string(),
                name: "Polish".to_string(),
                icon: "sparkles".to_string(),
                system_prompt: LEGACY_POLISH_PROMPT.to_string(),
            },
            TextTransform {
                id: "promptEngineer".to_string(),
                name: "Prompt Engineer".to_string(),
                icon: "blocks".to_string(),
                system_prompt: LEGACY_PROMPT_ENGINEER_PROMPT.to_string(),
            },
            TextTransform {
                id: "custom".to_string(),
                name: "Custom".to_string(),
                icon: "pen".to_string(),
                system_prompt: "Custom prompt.".to_string(),
            },
        ];

        settings.normalize_transforms();

        assert_eq!(settings.transforms[0].system_prompt, POLISH_PROMPT);
        assert_eq!(settings.transforms[1].system_prompt, PROMPT_ENGINEER_PROMPT);
        assert_eq!(settings.transforms[2].system_prompt, "Custom prompt.");
    }

    #[test]
    fn test_normalize_dock_preferences_repairs_invalid_values() {
        let mut settings = Settings::default();
        settings.dock_size = "huge".to_string();
        settings.dock_shape = "blob".to_string();
        settings.dock_color = "neon".to_string();
        settings.dock_position = "middle".to_string();
        settings.dock_inset = 87;
        settings.dock_width_offset = 83;
        settings.dock_height_offset = -15;

        settings.normalize_dock_preferences();

        assert_eq!(settings.dock_size, "regular");
        assert_eq!(settings.dock_shape, "pill");
        assert_eq!(settings.dock_color, "charcoal");
        assert_eq!(settings.dock_position, "bottom-center");
        assert_eq!(settings.dock_inset, 80);
        assert_eq!(settings.dock_width_offset, 80);
        assert_eq!(settings.dock_height_offset, -12);
    }

    #[test]
    fn test_normalize_language_preferences_repairs_invalid_values() {
        let mut settings = Settings::default();
        settings.transcript_languages = vec![
            "UR".to_string(),
            "invalid".to_string(),
            "en".to_string(),
            "ur".to_string(),
            " yue ".to_string(),
        ];

        settings.normalize_language_preferences();

        assert_eq!(settings.transcript_languages, vec!["ur", "en", "yue"]);
    }

    #[test]
    fn test_normalize_language_preferences_falls_back_to_english() {
        let mut settings = Settings::default();
        settings.transcript_languages = vec!["invalid".to_string()];

        settings.normalize_language_preferences();

        assert_eq!(settings.transcript_languages, vec!["en"]);
    }

    #[test]
    fn test_normalize_dock_preferences_allows_outward_offset() {
        let mut settings = Settings::default();
        settings.dock_inset = -34;

        settings.normalize_dock_preferences();

        assert_eq!(settings.dock_inset, -30);
    }

    #[test]
    fn test_normalize_model_preferences_repairs_invalid_values() {
        let mut settings = Settings::default();
        settings.transcript_model_mode = "manual".to_string();
        settings.transcript_model_id = "  whisper-large-v3  ".to_string();
        settings.transform_model_mode = "manual".to_string();
        settings.transform_model_id = "  llama-3.1-8b-instant  ".to_string();

        settings.normalize_model_preferences();

        assert_eq!(settings.transcript_model_mode, "default");
        assert_eq!(settings.transcript_model_id, "whisper-large-v3");
        assert_eq!(settings.transcript_model(), DEFAULT_TRANSCRIPT_MODEL_ID);
        assert_eq!(settings.transform_model_mode, "default");
        assert_eq!(settings.transform_model_id, "llama-3.1-8b-instant");
        assert_eq!(settings.transform_model(), DEFAULT_TRANSFORM_MODEL_ID);
    }
}
