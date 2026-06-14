# Typr

Typr is a macOS dictation app for people who want voice input to work anywhere they can type. Use a global hotkey or the floating dock, speak naturally, and Typr transcribes, optionally transforms, and inserts the final text into the active app.

Think of it as a Wispr Flow or WhisperFlow-style alternative, with more control over the transcription engine, LLM models, transforms, history, and floating dock behavior. Typr is not affiliated with Wispr AI or Wispr Flow.

## What It Does

- Dictate into any focused text field from a global hotkey or compact floating dock.
- Transcribe with local whisper.cpp models or Groq's hosted Whisper models.
- Paste the final text back into the selected app automatically.
- Apply configurable LLM transforms after transcription, such as Polish or Prompt Engineer.
- Keep raw transcripts, final text, transform details, and audio recordings in history.
- Support one or more transcription languages, with auto-detect when multiple languages are selected.
- Cancel silent takes automatically so accidental recordings do not waste transcription or transform calls.
- Customize the dock size, shape, color, position, edge inset, width, and height.
- Start automatically on login when enabled.

## Why Typr

Most dictation tools optimize for a fixed workflow. Typr is built for people who want the same fast voice-to-text loop, but with knobs exposed:

- Use local transcription when you want more control.
- Use Groq when you want hosted transcription and transforms.
- Add your own transforms with a name, icon, and system prompt.
- Choose default or custom model IDs separately for transcription and transforms.
- Review what happened later, including the raw transcript, transformed output, paste status, model choices, languages, and audio.

## Core Features

### Floating Dock

The floating dock is the primary control surface. It can stay close to the macOS Dock, move to different screen edges, and be tuned in 10 pixel or smaller increments depending on the setting. A visible border keeps it readable on light and dark backgrounds.

### Transcription Engines

Typr supports:

- Local transcription through the bundled `whisper-cpp` sidecar.
- Cloud transcription through Groq's OpenAI-compatible audio transcription endpoint.

Single-language mode passes the selected language directly to the transcription engine. Multi-language mode uses language auto-detection instead of forcing one language, which works better for mixed setups such as English and Urdu.

### Transforms

Transforms are configurable from settings. Each transform has:

- Name
- Icon
- System prompt

The defaults are:

- Polish: rewrite dictated text to be clearer and easier to read while preserving meaning and tone.
- Prompt Engineer: convert dictated notes into a structured prompt for an AI assistant.

### Model Control

Typr lets you keep the default model or enter a custom model ID for:

- Transcript generation
- Transform generation

This makes it easy to try new Groq-compatible models without changing code.

### History

The history page stores:

- Raw transcript
- Cleaned transcript
- Final inserted text
- Transform input and output
- Transform prompt and model
- Transcript model and engine
- Selected languages
- Paste status
- Audio recording

Canceled silent takes are skipped so history does not fill up with empty recordings.

### macOS Integration

Typr includes macOS-specific support for:

- Microphone permissions in dev and packaged builds.
- Accessibility-based paste simulation.
- Global shortcut capture.
- Start on login through a LaunchAgent.
- A native-feeling floating overlay.

## Permissions

Typr may ask macOS for:

- Microphone access, used to capture dictation audio.
- Accessibility access, used to paste the result into the active app.

If paste simulation fails, open System Settings and allow Typr under Privacy & Security -> Accessibility.

## Development

### Prerequisites

- macOS
- Node.js
- Rust
- Tauri CLI dependencies

Install frontend dependencies:

```bash
npm install
```

Run the Tauri app in development:

```bash
npm run tauri dev
```

Build the frontend:

```bash
npm run build
```

Build the macOS app bundle:

```bash
npm run tauri build -- --bundles app
```

## Local Whisper Models

Local transcription uses whisper.cpp model files stored in the app data directory. Models can be downloaded from the app settings.

The bundled sidecar is configured in `src-tauri/tauri.conf.json` as:

```json
"externalBin": ["binaries/whisper-cpp"]
```

## Data Storage

Settings and history are stored under the app config directory:

```text
com.typr.app/
  config.json
  history.json
  history-audio/
```

On macOS this is typically under the user's Library application support/config location.

## Project Structure

```text
src/
  main.ts              Settings UI and history UI
  overlay.ts           Floating dock behavior
  notes.ts             Notes window
  transforms.ts        Default transform definitions
  languages.ts         Supported transcription languages

src-tauri/src/
  audio.rs             Microphone capture, waveform levels, WAV output
  recorder.rs          Recording state machine and transcript pipeline
  transcribe_local.rs  whisper.cpp transcription
  transcribe_groq.rs   Groq transcription and transforms
  settings.rs          Persistent app settings
  history.rs           Transcript and audio history
  paste.rs             macOS paste simulation
  main.rs              Tauri commands, windows, hotkeys, login item
```

## Status

Typr is an active macOS desktop app project. The current focus is making dictation feel instant, reliable, configurable, and quiet enough to live in the background all day.
