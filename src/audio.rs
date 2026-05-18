use crate::error::Result;
use crate::models::AudioFormat;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

/// TTS provider trait. Implement this for any cloud TTS service.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Synthesize text to audio bytes. Returns (audio_bytes, format).
    async fn synthesize(&self, text: &str, voice: Option<&str>) -> Result<(Vec<u8>, AudioFormat)>;
}

/// A registered sound: pre-loaded audio bytes ready for playback.
#[derive(Debug, Clone)]
pub struct SoundDef {
    pub data: Vec<u8>,
    pub format: AudioFormat,
    pub volume: f64,
}

/// Registry mapping action names to sounds, plus optional TTS.
pub struct SoundRegistry {
    sounds: HashMap<String, SoundDef>,
    tts: Option<Box<dyn TtsProvider>>,
}

impl SoundRegistry {
    pub fn new() -> Self {
        Self {
            sounds: HashMap::new(),
            tts: None,
        }
    }

    /// Register a sound file for an action.
    pub fn register_file(&mut self, action: &str, path: &Path, volume: f64) -> Result<()> {
        let data = std::fs::read(path)?;
        let format = guess_format(path)?;
        self.sounds.insert(
            action.to_string(),
            SoundDef { data, format, volume },
        );
        Ok(())
    }

    /// Register raw audio bytes for an action.
    pub fn register_bytes(
        &mut self,
        action: &str,
        data: Vec<u8>,
        format: AudioFormat,
        volume: f64,
    ) {
        self.sounds
            .insert(action.to_string(), SoundDef { data, format, volume });
    }

    /// Set the TTS provider for speech synthesis.
    pub fn set_tts(&mut self, provider: Box<dyn TtsProvider>) {
        self.tts = Some(provider);
    }

    /// Get audio for an action. Returns None if no sound registered.
    pub fn get(&self, action: &str) -> Option<&SoundDef> {
        self.sounds.get(action)
    }

    /// Check if a sound is registered for an action.
    pub fn has(&self, action: &str) -> bool {
        self.sounds.contains_key(action)
    }

    /// Synthesize speech via TTS. Returns None if no TTS provider configured.
    pub async fn speak(&self, text: &str) -> Result<Option<(Vec<u8>, AudioFormat)>> {
        match &self.tts {
            Some(provider) => {
                let (bytes, fmt) = provider.synthesize(text, None).await?;
                Ok(Some((bytes, fmt)))
            }
            None => Ok(None),
        }
    }

    /// Remove a registered sound.
    pub fn unregister(&mut self, action: &str) {
        self.sounds.remove(action);
    }

    /// Clear all registered sounds.
    pub fn clear(&mut self) {
        self.sounds.clear();
    }
}

impl Default for SoundRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn guess_format(path: &Path) -> Result<AudioFormat> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav") => Ok(AudioFormat::Wav),
        Some("ogg") => Ok(AudioFormat::Ogg),
        Some("mp3") => Ok(AudioFormat::Mp3),
        _ => Err(crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unsupported audio format: {}", path.display()),
        ))),
    }
}

// ─── Built-in TTS Providers ─────────────────────────────────────────

/// Azure Cognitive Services TTS.
pub struct AzureTts {
    pub api_key: String,
    pub region: String,
    pub voice: String,
    http: reqwest::Client,
}

impl AzureTts {
    pub fn new(api_key: String, region: String, voice: String) -> Self {
        Self {
            api_key,
            region,
            voice,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for AzureTts {
    async fn synthesize(&self, text: &str, voice: Option<&str>) -> Result<(Vec<u8>, AudioFormat)> {
        let voice = voice.unwrap_or(&self.voice);
        let url = format!(
            "https://{}.tts.speech.microsoft.com/cognitiveservices/v1",
            self.region
        );

        let ssml = format!(
            r#"<speak version='1.0' xml:lang='en-US'><voice name='{}'>{}</voice></speak>"#,
            voice,
            xml_escape(text)
        );

        let resp = self
            .http
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", "ogg-48khz-16bit-mono-opus")
            .body(ssml)
            .send()
            .await?;

        let bytes = resp.bytes().await?.to_vec();
        Ok((bytes, AudioFormat::Ogg))
    }
}

/// ElevenLabs TTS.
pub struct ElevenLabsTts {
    pub api_key: String,
    pub voice_id: String,
    http: reqwest::Client,
}

impl ElevenLabsTts {
    pub fn new(api_key: String, voice_id: String) -> Self {
        Self {
            api_key,
            voice_id,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsTts {
    async fn synthesize(&self, text: &str, voice: Option<&str>) -> Result<(Vec<u8>, AudioFormat)> {
        let voice_id = voice.unwrap_or(&self.voice_id);
        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            voice_id
        );

        let body = serde_json::json!({
            "text": text,
            "model_id": "eleven_monolingual_v1",
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.75,
            }
        });

        let resp = self
            .http
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let bytes = resp.bytes().await?.to_vec();
        Ok((bytes, AudioFormat::Mp3))
    }
}

/// No-op TTS provider (returns empty audio).
pub struct NoopTts;

#[async_trait]
impl TtsProvider for NoopTts {
    async fn synthesize(&self, _text: &str, _voice: Option<&str>) -> Result<(Vec<u8>, AudioFormat)> {
        Ok((vec![], AudioFormat::Wav))
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
