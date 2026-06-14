use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::time::{Duration, Instant};

const AUDIO_LEVEL_INTERVAL: Duration = Duration::from_millis(33);
const TRANSCRIPTION_TARGET_RMS: f32 = 0.14;
const TRANSCRIPTION_PEAK_HEADROOM: f32 = 0.94;
const TRANSCRIPTION_MAX_GAIN: f32 = 4.0;
const SILENT_CANCEL_MIN_DURATION_SECS: f32 = 0.45;
const SILENT_CANCEL_TRAILING_SECS: f32 = 2.0;
const SILENT_CANCEL_MAX_SPEECH_SECS: f32 = 0.45;
const SILENT_CANCEL_FRAME_MS: u32 = 100;
const SILENCE_RMS_THRESHOLD: f32 = 0.003;
const SPEECH_RMS_THRESHOLD: f32 = 0.006;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MicDevice {
    pub name: String,
    pub is_default: bool,
}

pub fn list_microphones() -> Vec<MicDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut devices = Vec::new();
    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            if let Ok(name) = device.name() {
                devices.push(MicDevice {
                    is_default: name == default_name,
                    name,
                });
            }
        }
    }
    devices
}

/// Wrapper to make cpal::Stream usable across threads.
/// SAFETY: cpal::Stream on macOS (CoreAudio) is thread-safe in practice;
/// we only access it behind a Mutex to start/stop recording.
struct SendStream(cpal::Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

impl Drop for SendStream {
    fn drop(&mut self) {
        if let Err(error) = self.0.pause() {
            eprintln!("[Typr] Failed to pause audio stream: {}", error);
        }
    }
}

pub struct AudioRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<SendStream>,
    source_sample_rate: u32,
    source_channels: u16,
}

pub struct RecordingSaveResult {
    pub should_cancel_transcription: bool,
}

fn audio_level_for_samples(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let mut sum_squares = 0.0;
    let mut peak = 0.0_f32;

    for sample in samples {
        let amplitude = sample.abs().min(1.0);
        sum_squares += amplitude * amplitude;
        peak = peak.max(amplitude);
    }

    let rms = (sum_squares / samples.len() as f32).sqrt();
    ((rms * 4.8).max(peak * 0.72)).clamp(0.0, 1.0)
}

fn transcription_gain_for_samples(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 1.0;
    }

    let mut sum_squares = 0.0;
    let mut peak = 0.0_f32;

    for sample in samples {
        let amplitude = sample.abs().min(1.0);
        sum_squares += amplitude * amplitude;
        peak = peak.max(amplitude);
    }

    if peak <= f32::EPSILON {
        return 1.0;
    }

    let rms = (sum_squares / samples.len() as f32).sqrt();
    let rms_gain = if rms > f32::EPSILON {
        TRANSCRIPTION_TARGET_RMS / rms
    } else {
        TRANSCRIPTION_MAX_GAIN
    };
    let peak_gain = TRANSCRIPTION_PEAK_HEADROOM / peak;

    rms_gain.min(peak_gain).clamp(1.0, TRANSCRIPTION_MAX_GAIN)
}

fn normalize_for_transcription(samples: &[f32]) -> Vec<f32> {
    let gain = transcription_gain_for_samples(samples);

    samples
        .iter()
        .map(|sample| (sample * gain).clamp(-1.0, 1.0))
        .collect()
}

fn rms_for_samples(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_squares = samples
        .iter()
        .map(|sample| {
            let amplitude = sample.abs().min(1.0);
            amplitude * amplitude
        })
        .sum::<f32>();

    (sum_squares / samples.len() as f32).sqrt()
}

fn speech_duration_secs(samples: &[f32], sample_rate: u32) -> f32 {
    if samples.is_empty() || sample_rate == 0 {
        return 0.0;
    }

    let frame_len = ((sample_rate * SILENT_CANCEL_FRAME_MS) / 1000).max(1) as usize;
    samples
        .chunks(frame_len)
        .filter(|frame| rms_for_samples(frame) >= SPEECH_RMS_THRESHOLD)
        .map(|frame| frame.len() as f32 / sample_rate as f32)
        .sum()
}

fn trailing_silence_secs(samples: &[f32], sample_rate: u32) -> f32 {
    if samples.is_empty() || sample_rate == 0 {
        return 0.0;
    }

    let frame_len = ((sample_rate * SILENT_CANCEL_FRAME_MS) / 1000).max(1) as usize;
    let mut end = samples.len();
    let mut silent_samples = 0;

    while end > 0 {
        let start = end.saturating_sub(frame_len);
        let frame = &samples[start..end];
        if rms_for_samples(frame) > SILENCE_RMS_THRESHOLD {
            break;
        }

        silent_samples += frame.len();
        end = start;
    }

    silent_samples as f32 / sample_rate as f32
}

fn should_cancel_silent_take(samples: &[f32], sample_rate: u32) -> bool {
    if samples.is_empty() || sample_rate == 0 {
        return false;
    }

    let duration_secs = samples.len() as f32 / sample_rate as f32;
    if duration_secs < SILENT_CANCEL_MIN_DURATION_SECS {
        return false;
    }

    let speech_secs = speech_duration_secs(samples, sample_rate);
    if speech_secs <= f32::EPSILON {
        return true;
    }

    speech_secs <= SILENT_CANCEL_MAX_SPEECH_SECS
        && trailing_silence_secs(samples, sample_rate) >= SILENT_CANCEL_TRAILING_SECS
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            source_sample_rate: 48000,
            source_channels: 1,
        }
    }

    pub fn start(
        &mut self,
        mic_name: &str,
        level_sender: Option<Sender<f32>>,
    ) -> Result<(), String> {
        // Clear any leftover samples from previous recording
        self.samples.lock().unwrap().clear();

        let host = cpal::default_host();

        let device = if mic_name == "default" {
            host.default_input_device()
                .ok_or("No default input device found")?
        } else {
            host.input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().map(|n| n == mic_name).unwrap_or(false))
                .ok_or(format!("Microphone '{}' not found", mic_name))?
        };

        // Use the device's default config instead of forcing 16kHz
        let default_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        println!(
            "[Typr] Mic config: {}Hz, {} channels",
            sample_rate, channels
        );

        self.source_sample_rate = sample_rate;
        self.source_channels = channels;

        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let samples = self.samples.clone();
        let mut last_level_emit = Instant::now() - AUDIO_LEVEL_INTERVAL;
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Some(sender) = level_sender.as_ref() {
                        if last_level_emit.elapsed() >= AUDIO_LEVEL_INTERVAL {
                            let _ = sender.send(audio_level_for_samples(data));
                            last_level_emit = Instant::now();
                        }
                    }

                    let mut buf = samples.lock().unwrap();
                    buf.extend_from_slice(data);
                },
                |err| {
                    eprintln!("[Typr] Audio stream error: {}", err);
                },
                None,
            )
            .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(SendStream(stream));
        println!("[Typr] Audio recording started");
        Ok(())
    }

    pub fn stop_and_save(&mut self, output_path: &PathBuf) -> Result<RecordingSaveResult, String> {
        drop(self.stream.take()); // Drop pauses and stops the stream.
        println!("[Typr] Audio recording stopped");

        let samples = self.samples.lock().unwrap();
        if samples.is_empty() {
            return Err("No audio captured".to_string());
        }

        println!("[Typr] Captured {} raw samples", samples.len());

        // Convert to mono if multi-channel
        let mono: Vec<f32> = if self.source_channels > 1 {
            samples
                .chunks(self.source_channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                .collect()
        } else {
            samples.clone()
        };

        // Downsample to 16kHz for whisper.cpp
        let resampled = resample(&mono, self.source_sample_rate, 16000);
        let should_cancel_transcription = should_cancel_silent_take(&resampled, 16000);
        let normalized = normalize_for_transcription(&resampled);
        println!("[Typr] Resampled to {} samples at 16kHz", resampled.len());
        println!(
            "[Typr] Applied transcription gain {:.2}x",
            transcription_gain_for_samples(&resampled)
        );
        if should_cancel_transcription {
            println!("[Typr] Silent take detected, skipping transcription");
        }

        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(output_path, spec).map_err(|e| e.to_string())?;
        for &sample in normalized.iter() {
            let amplitude = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(amplitude).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;

        drop(samples);
        self.samples.lock().unwrap().clear();

        println!("[Typr] WAV saved to {:?}", output_path);
        Ok(RecordingSaveResult {
            should_cancel_transcription,
        })
    }

    pub fn cancel(&mut self) {
        drop(self.stream.take());
        self.samples.lock().unwrap().clear();
        println!("[Typr] Audio recording canceled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_level_empty_samples_is_silent() {
        assert_eq!(audio_level_for_samples(&[]), 0.0);
    }

    #[test]
    fn test_audio_level_tracks_louder_samples() {
        let quiet = audio_level_for_samples(&[0.01, -0.01, 0.02, -0.02]);
        let loud = audio_level_for_samples(&[0.4, -0.4, 0.6, -0.6]);

        assert!(quiet > 0.0);
        assert!(loud > quiet);
        assert!(loud <= 1.0);
    }

    #[test]
    fn test_transcription_gain_boosts_quiet_audio() {
        let gain = transcription_gain_for_samples(&[0.01, -0.01, 0.02, -0.02]);

        assert!(gain > 1.0);
        assert!(gain <= TRANSCRIPTION_MAX_GAIN);
    }

    #[test]
    fn test_transcription_gain_leaves_loud_audio_near_original() {
        let gain = transcription_gain_for_samples(&[0.5, -0.5, 0.6, -0.6]);

        assert_eq!(gain, 1.0);
    }

    #[test]
    fn test_normalize_for_transcription_clamps_samples() {
        let normalized = normalize_for_transcription(&[0.8, -0.8]);

        assert!(normalized.iter().all(|sample| sample.abs() <= 1.0));
    }

    #[test]
    fn test_silent_take_cancels_transcription() {
        let samples = vec![0.0; 16000 * 3];

        assert!(should_cancel_silent_take(&samples, 16000));
    }

    #[test]
    fn test_background_noise_take_cancels_transcription() {
        let samples = vec![0.004; 16000 * 3];

        assert!(should_cancel_silent_take(&samples, 16000));
    }

    #[test]
    fn test_short_real_speech_then_silence_cancels_transcription() {
        let mut samples = vec![0.03; 16000 / 4];
        samples.extend(vec![0.0; 16000 * 3]);

        assert!(should_cancel_silent_take(&samples, 16000));
    }

    #[test]
    fn test_speech_followed_by_pause_keeps_transcription() {
        let mut samples = vec![0.03; 16000];
        samples.extend(vec![0.0; 16000 * 3]);

        assert!(!should_cancel_silent_take(&samples, 16000));
    }

    #[test]
    fn test_quiet_speech_keeps_transcription() {
        let samples = vec![0.009; 16000 * 2];

        assert!(!should_cancel_silent_take(&samples, 16000));
    }
}

/// Simple linear interpolation resampler
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        let sample = if idx + 1 < samples.len() {
            samples[idx] as f64 * (1.0 - frac) + samples[idx + 1] as f64 * frac
        } else {
            samples[idx.min(samples.len() - 1)] as f64
        };

        output.push(sample as f32);
    }

    output
}
