use serde::{Deserialize, Serialize};

/// Frame index for a sprite sheet: layout info plus per-frame pixel rects.
/// Does not hold image data — the runtime only needs coordinates, not pixels.
#[derive(Debug)]
pub struct SpriteSheet {
    pub layout: FrameLayout,
    /// frames[row][col] — each entry is the pixel rect of that frame.
    pub frames: Vec<Vec<FrameRect>>,
}

impl SpriteSheet {
    /// Create a SpriteSheet from a layout. Builds the frame rect grid automatically.
    pub fn new(layout: FrameLayout) -> Self {
        let frames = (0..layout.rows)
            .map(|r| {
                (0..layout.columns)
                    .map(|c| FrameRect {
                        x: c * layout.cell_width,
                        y: r * layout.cell_height,
                        width: layout.cell_width,
                        height: layout.cell_height,
                    })
                    .collect()
            })
            .collect();
        Self { layout, frames }
    }
}

/// Grid layout descriptor for a sprite sheet.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrameLayout {
    pub columns: u32,
    pub rows: u32,
    pub cell_width: u32,
    pub cell_height: u32,
}

impl Default for FrameLayout {
    fn default() -> Self {
        Self {
            columns: 8,
            rows: 9,
            cell_width: 192,
            cell_height: 208,
        }
    }
}

/// The pet kind as reported by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PetKind {
    #[default]
    Person,
    Animal,
    Object,
    Creature,
}

/// Validation report from the codex-pets API.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub manifest_id: String,
    pub atlas_size: String,
    pub cell_size: String,
    pub states_detected: u32,
    pub manifest_bytes: u64,
    pub spritesheet_bytes: u64,
}

/// Full pet metadata as returned by the listing API.
/// Fields that are not present in all providers use `#[serde(default)]`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PetMeta {
    #[serde(alias = "slug")]
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub spritesheet_path: String,
    #[serde(default)]
    pub kind: PetKind,
    #[serde(default)]
    pub owner_id: String,
    #[serde(default)]
    pub owner_handle: String,
    #[serde(default, alias = "author_name")]
    pub owner_name: String,
    #[serde(default, alias = "published_at")]
    pub uploaded_at: String,
    #[serde(default)]
    pub view_count: u64,
    #[serde(default)]
    pub download_count: u64,
    #[serde(default)]
    pub like_count: u64,
    #[serde(default)]
    pub comment_count: u64,
    #[serde(default)]
    pub liked_by_me: bool,
    #[serde(default)]
    pub owner_shadowbanned: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub spritesheet_url: String,
    #[serde(default)]
    pub poster_url: String,
    #[serde(default)]
    pub preview_url: String,
    #[serde(default)]
    pub share_image_url: String,
    #[serde(default)]
    pub download_url: String,
    #[serde(default)]
    pub validation_report: Option<ValidationReport>,
}

impl PetMeta {
    /// Create an empty PetMeta with default values. Useful for local/offline mode.
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            description: String::new(),
            spritesheet_path: String::new(),
            kind: PetKind::default(),
            owner_id: String::new(),
            owner_handle: String::new(),
            owner_name: String::new(),
            uploaded_at: String::new(),
            view_count: 0,
            download_count: 0,
            like_count: 0,
            comment_count: 0,
            liked_by_me: false,
            owner_shadowbanned: false,
            tags: Vec::new(),
            spritesheet_url: String::new(),
            poster_url: String::new(),
            preview_url: String::new(),
            share_image_url: String::new(),
            download_url: String::new(),
            validation_report: None,
        }
    }
}

/// Paginated API response for pet listing.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PetListResponse {
    pub pets: Vec<PetMeta>,
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
    pub total_pages: u32,
}

/// A single frame's position within the sprite sheet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// An action (animation) definition. Maps to a row in the sprite sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDef {
    pub name: String,
    pub row: u32,
    pub frame_count: u32,
    pub frame_duration_ms: u64,
    pub looping: bool,
    pub interruptible: bool,
    /// Pause at last frame before wrapping to frame 0 (looping actions only).
    #[serde(default)]
    pub loop_rest_ms: Option<u64>,
    /// Hold last frame before auto-reverting to idle (non-looping actions only).
    #[serde(default)]
    pub last_frame_hold_ms: Option<u64>,
}

/// Current state of the pet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetState {
    pub pet_id: String,
    pub action: String,
    pub frame_index: u32,
    pub frame_rect: FrameRect,
    pub position: Position,
    pub facing: Facing,
    pub bubble: Option<BubbleSnapshot>,
    pub stats: PetStats,
    pub mood_label: String,
}

/// Serializable snapshot of a bubble for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BubbleSnapshot {
    pub text: String,
    pub kind: BubbleKind,
    pub typing_animation: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Facing {
    Left,
    Right,
}

/// Audio format for sound playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Wav,
    Ogg,
    Mp3,
}

/// Config persisted to pet.json — describes a downloaded pet's spritesheet layout and actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetConfig {
    pub id: String,
    pub display_name: String,
    pub spritesheet_path: String,
    pub layout: FrameLayout,
    pub actions: Vec<ActionDef>,
}

/// Result returned by load_pet to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadPetResult {
    pub config: PetConfig,
    /// Raw spritesheet image bytes (WebP). Sent once on load, not persisted in pet.json.
    pub spritesheet_bytes: Vec<u8>,
}

/// Command emitted from backend to frontend. The frontend is a dumb renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PetCommand {
    Render {
        action: String,
        frame_index: u32,
        facing: String,
        x: f64,
        y: f64,
        scale: f64,
    },
    Audio {
        audio_bytes: Vec<u8>,
        format: AudioFormat,
        volume: f64,
    },
    Bubble {
        text: String,
        kind: String,
        duration_ms: u64,
    },
    DismissBubble,
    ActionFinished {
        action: String,
    },
}

/// Position and facing info returned by get_position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionInfo {
    pub x: f64,
    pub y: f64,
    pub facing: Facing,
}

/// Pet mood stats.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PetStats {
    pub happiness: f64,
    pub energy: f64,
    pub social: f64,
    pub boredom: f64,
}

impl Default for PetStats {
    fn default() -> Self {
        Self {
            happiness: 70.0,
            energy: 80.0,
            social: 50.0,
            boredom: 30.0,
        }
    }
}

impl PetStats {
    pub fn clamp(&mut self) {
        self.happiness = self.happiness.clamp(0.0, 100.0);
        self.energy = self.energy.clamp(0.0, 100.0);
        self.social = self.social.clamp(0.0, 100.0);
        self.boredom = self.boredom.clamp(0.0, 100.0);
    }

    pub fn mood_score(&self) -> f64 {
        (self.happiness * 0.4 + self.energy * 0.2 + self.social * 0.3 - self.boredom * 0.1)
            .clamp(0.0, 100.0)
    }

    pub fn mood_label(&self) -> &'static str {
        match self.mood_score() as u32 {
            80..=100 => "ecstatic",
            60..=79 => "happy",
            40..=59 => "neutral",
            20..=39 => "sad",
            _ => "depressed",
        }
    }
}

/// Bubble display kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BubbleKind {
    Speech,
    Thought,
    Action,
    System,
}

/// Behavior engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorConfig {
    /// Ms of no interaction before entering ambient mode.
    pub idle_timeout_ms: u64,
    /// Min ms between ambient actions.
    pub ambient_interval_min_ms: u64,
    /// Max ms between ambient actions.
    pub ambient_interval_max_ms: u64,
    /// Available ambient actions with weights and conditions.
    pub ambient_actions: Vec<AmbientAction>,
    /// 0.0-1.0, how much mood affects behavior selection.
    pub mood_influence: f64,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            idle_timeout_ms: 30_000,
            ambient_interval_min_ms: 15_000,
            ambient_interval_max_ms: 120_000,
            mood_influence: 0.5,
            ambient_actions: vec![
                AmbientAction {
                    action: "waving".into(),
                    weight: 3.0,
                    bubble: Some("*waves*".into()),
                    min_mood: Some(40.0),
                    max_mood: None,
                    cooldown_ms: 30_000,
                },
                AmbientAction {
                    action: "jumping".into(),
                    weight: 2.0,
                    bubble: None,
                    min_mood: Some(50.0),
                    max_mood: None,
                    cooldown_ms: 20_000,
                },
                AmbientAction {
                    action: "waiting".into(),
                    weight: 1.0,
                    bubble: Some("...".into()),
                    min_mood: None,
                    max_mood: Some(60.0),
                    cooldown_ms: 10_000,
                },
                AmbientAction {
                    action: "idle".into(),
                    weight: 2.0,
                    bubble: None,
                    min_mood: None,
                    max_mood: None,
                    cooldown_ms: 5_000,
                },
            ],
        }
    }
}

/// An ambient action with selection weight and mood conditions.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AmbientAction {
    /// Action name to play.
    pub action: String,
    /// Base selection weight.
    pub weight: f64,
    /// Optional bubble text when this triggers.
    pub bubble: Option<String>,
    /// Only play if mood score >= this.
    pub min_mood: Option<f64>,
    /// Only play if mood score <= this.
    pub max_mood: Option<f64>,
    /// Don't repeat within this window (ms).
    pub cooldown_ms: u64,
}

/// Saved pet snapshot for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetSnapshot {
    pub pet_id: String,
    pub position: Position,
    pub facing: Facing,
    pub stats: PetStats,
    pub current_action: String,
    pub saved_at: String,
}

/// User-triggerable pet events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PetEvent {
    Idle,
    Walk { direction: Facing },
    DragStart,
    DragMove { x: f64, y: f64 },
    DragDrop,
    Click,
    DoubleClick,
    Sleep,
    Wake,
    Custom(String),
}

/// Action sequence for choreography.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSequence {
    pub steps: Vec<SequenceStep>,
    pub repeat: SequenceRepeat,
    pub on_complete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SequenceRepeat {
    Once,
    Loop,
    LoopN(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceStep {
    pub action: String,
    pub loops: Option<u32>,
    pub sound: Option<SoundTrigger>,
    pub bubble: Option<BubbleDef>,
    pub wait_for_complete: bool,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundTrigger {
    pub sound_id: String,
    pub volume: f64,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BubbleDef {
    pub text: String,
    pub kind: BubbleKind,
    pub duration_ms: u64,
    pub typing_animation: bool,
}

/// Mood configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoodConfig {
    pub decay_interval_ms: u64,
    pub happiness_decay: f64,
    pub energy_decay: f64,
    pub social_decay: f64,
    pub boredom_increase: f64,
    pub interaction_boost: f64,
}

impl Default for MoodConfig {
    fn default() -> Self {
        Self {
            decay_interval_ms: 60_000,
            happiness_decay: 0.5,
            energy_decay: 0.3,
            social_decay: 0.4,
            boredom_increase: 0.6,
            interaction_boost: 5.0,
        }
    }
}
