use crate::error::Result;
use crate::models::{PetConfig, PetListResponse, PetMeta};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

/// Global cache root, set once during plugin init.
static APP_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set the application-scoped cache directory. Called once during plugin init.
pub(crate) fn set_app_cache_dir(dir: PathBuf) {
    let _ = APP_CACHE_DIR.set(dir);
}

/// Response format from the pet metadata API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    /// Full `PetMeta` JSON (codex-pets.net style).
    FullMeta,
    /// Abbreviated format that needs normalization (codexpet.xyz style).
    AbbreviatedMeta,
}

/// A pet resource provider — knows how to talk to a specific API.
///
/// # Pre-defined providers
///
/// ```rust
/// use tauri_plugin_sprite_pet::ResourceProvider;
///
/// let p1 = ResourceProvider::codex_pets();
/// let p2 = ResourceProvider::codexpet_xyz();
/// ```
///
/// # Custom provider
///
/// ```rust
/// use tauri_plugin_sprite_pet::ResourceProvider;
///
/// // Spritesheet URL defaults to {base}/api/pets/{id}/spritesheet
/// let p = ResourceProvider::custom("https://my-api.example.com");
///
/// // Or specify a custom template (use {id} as placeholder)
/// let p = ResourceProvider::custom("https://my-api.example.com")
///     .with_spritesheet_url("https://cdn.example.com/sprites/{id}.webp");
/// ```
#[derive(Debug, Clone)]
pub struct ResourceProvider {
    pub base_url: String,
    pub(crate) response_format: ResponseFormat,
    pub(crate) spritesheet_url_template: Option<String>,
}

impl ResourceProvider {
    /// codex-pets.net — full metadata API with spritesheet URL in response.
    pub fn codex_pets() -> Self {
        Self {
            base_url: "https://codex-pets.net".into(),
            response_format: ResponseFormat::FullMeta,
            spritesheet_url_template: None,
        }
    }

    /// codexpet.xyz — abbreviated metadata, spritesheet at `/api/pets/{id}/spritesheet`.
    pub fn codexpet_xyz() -> Self {
        Self {
            base_url: "https://codexpet.xyz".into(),
            response_format: ResponseFormat::AbbreviatedMeta,
            spritesheet_url_template: None,
        }
    }

    /// Custom provider. Uses `FullMeta` format by default.
    ///
    /// Spritesheet URL defaults to `{base}/api/pets/{id}/spritesheet`.
    pub fn custom(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').into(),
            response_format: ResponseFormat::FullMeta,
            spritesheet_url_template: None,
        }
    }

    /// Set the spritesheet URL template. Use `{id}` as a placeholder for the pet ID.
    ///
    /// If not set, defaults to `{base}/api/pets/{id}/spritesheet`.
    pub fn with_spritesheet_url(mut self, template: &str) -> Self {
        self.spritesheet_url_template = Some(template.into());
        self
    }

    /// Override the response format (default: `FullMeta`).
    pub fn with_response_format(mut self, format: ResponseFormat) -> Self {
        self.response_format = format;
        self
    }

    /// Metadata API URL for a given pet ID.
    pub(crate) fn meta_url(&self, id: &str) -> String {
        format!("{}/api/pets/{}", self.base_url, id)
    }

    /// Spritesheet download URL for a given pet ID.
    pub(crate) fn spritesheet_url(&self, id: &str) -> String {
        if let Some(tmpl) = &self.spritesheet_url_template {
            return tmpl.replace("{id}", id);
        }
        format!("{}/api/pets/{}/spritesheet", self.base_url, id)
    }
}

/// Configuration for the resource module.
#[derive(Debug, Clone)]
pub struct ResourceConfig {
    pub provider: ResourceProvider,
    pub cache_dir: PathBuf,
    pub request_timeout: Duration,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            provider: ResourceProvider::codex_pets(),
            cache_dir: cache_dir(),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// Cache root directory for the plugin.
///
/// When used as a Tauri plugin, this is automatically set to
/// `{app_local_data_dir}/sprite-pet` (e.g. `%LOCALAPPDATA%/{identifier}/sprite-pet`
/// on Windows), so each application's cache is isolated.
///
/// When used as a standalone library without calling `init()`, falls back to
/// `{system_cache_dir}/sprite-pet`.
pub fn cache_dir() -> PathBuf {
    APP_CACHE_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("sprite-pet")
        })
}

/// Cache directory for a specific pet under the default cache root.
pub fn pet_cache_dir(pet_id: &str) -> PathBuf {
    cache_dir().join(pet_id)
}

/// Path to a pet's cached config file (`sprite-pet.json`).
fn cached_config_path_in(base: &Path, pet_id: &str) -> PathBuf {
    base.join(pet_id).join("sprite-pet.json")
}

/// Lightweight struct for deserializing abbreviated pet metadata (codexpet.xyz).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AbbreviatedPetMeta {
    slug: Option<String>,
    id: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    author_name: Option<String>,
    published_at: Option<String>,
    view_count: Option<u64>,
    download_count: Option<u64>,
    like_count: Option<u64>,
    tags: Option<Vec<String>>,
    spritesheet_url: Option<String>,
    download_url: Option<String>,
}

impl AbbreviatedPetMeta {
    fn into_pet_meta(self, base_url: &str) -> PetMeta {
        let id = self.slug.or(self.id).unwrap_or_default();
        let spritesheet_url = self
            .spritesheet_url
            .unwrap_or_else(|| format!("{}/api/pets/{}/spritesheet", base_url, id));
        PetMeta {
            id: id.clone(),
            display_name: self.display_name.unwrap_or_else(|| id.clone()),
            description: self.description.unwrap_or_default(),
            spritesheet_path: String::new(),
            kind: crate::models::PetKind::default(),
            owner_id: String::new(),
            owner_handle: self.author_name.clone().unwrap_or_default(),
            owner_name: self.author_name.unwrap_or_default(),
            uploaded_at: self.published_at.unwrap_or_default(),
            view_count: self.view_count.unwrap_or(0),
            download_count: self.download_count.unwrap_or(0),
            like_count: self.like_count.unwrap_or(0),
            comment_count: 0,
            liked_by_me: false,
            owner_shadowbanned: false,
            tags: self.tags.unwrap_or_default(),
            spritesheet_url,
            poster_url: String::new(),
            preview_url: String::new(),
            share_image_url: String::new(),
            download_url: self.download_url.unwrap_or_default(),
            validation_report: None,
        }
    }
}

/// Fetches pet listings, downloads and caches sprite sheets.
pub struct ResourceClient {
    config: ResourceConfig,
    http: reqwest::Client,
}

impl ResourceClient {
    pub fn new(config: ResourceConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .build()?;
        Ok(Self { config, http })
    }

    /// Get a reference to the current config.
    pub fn config(&self) -> &ResourceConfig {
        &self.config
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> PathBuf {
        self.config.cache_dir.clone()
    }

    /// Fetch paginated pet listing (codex-pets.net only).
    pub async fn list_pets(&self, page: u32, page_size: u32) -> Result<PetListResponse> {
        let url = format!(
            "{}/api/pets?page={}&pageSize={}",
            self.config.provider.base_url, page, page_size
        );
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let data = resp.json::<PetListResponse>().await?;
        Ok(data)
    }

    /// Search pets by query string (codex-pets.net only).
    pub async fn search_pets(
        &self,
        query: &str,
        page: u32,
        page_size: u32,
    ) -> Result<PetListResponse> {
        let url = format!(
            "{}/api/pets?page={}&pageSize={}&q={}",
            self.config.provider.base_url,
            page,
            page_size,
            urlencoding::encode(query)
        );
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let data = resp.json::<PetListResponse>().await?;
        Ok(data)
    }

    /// Get full metadata for a single pet by ID.
    pub async fn get_pet(&self, id: &str) -> Result<PetMeta> {
        let url = self.config.provider.meta_url(id);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let data = resp.json::<serde_json::Value>().await?;
        let pet_value = data.get("pet").cloned().unwrap_or(data);

        let mut meta: PetMeta = match self.config.provider.response_format {
            ResponseFormat::FullMeta => serde_json::from_value(pet_value)?,
            ResponseFormat::AbbreviatedMeta => {
                let raw: AbbreviatedPetMeta = serde_json::from_value(pet_value)?;
                raw.into_pet_meta(&self.config.provider.base_url)
            }
        };

        // Fill in spritesheet_url if the API didn't return one
        if meta.spritesheet_url.is_empty() {
            meta.spritesheet_url = self.config.provider.spritesheet_url(id);
        }

        Ok(meta)
    }

    /// Download and cache a pet's sprite sheet. Returns the local path.
    pub async fn fetch_spritesheet(&self, pet_id: &str) -> Result<PathBuf> {
        // Check for an existing cached file with any supported extension
        let pet_dir = self.config.cache_dir.join(pet_id);
        if let Some(existing) = find_cached_spritesheet(&pet_dir) {
            return Ok(existing);
        }

        if let Some(parent) = pet_dir.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Prefer the provider's direct spritesheet URL over the metadata URL
        let url = self.config.provider.spritesheet_url(pet_id);

        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let ext = extension_from_content_type(
            resp.headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        );
        let bytes = resp.bytes().await?;

        let dest = pet_dir.join(format!("spritesheet.{ext}"));
        tokio::fs::write(&dest, &bytes).await?;
        Ok(dest)
    }

    /// Get the cache path for a pet's spritesheet.
    ///
    /// Scans the pet's cache directory for an existing file with a supported
    /// image extension. Falls back to `spritesheet.webp` if none is found.
    pub fn cached_spritesheet_path(&self, pet_id: &str) -> PathBuf {
        let pet_dir = self.config.cache_dir.join(pet_id);
        find_cached_spritesheet(&pet_dir).unwrap_or_else(|| pet_dir.join("spritesheet.webp"))
    }

    /// Get the cache path for a pet's config file.
    pub fn cached_config_path(&self, pet_id: &str) -> PathBuf {
        cached_config_path_in(&self.config.cache_dir, pet_id)
    }

    /// Get the absolute path to the cached spritesheet as a string.
    pub fn spritesheet_abs_path(&self, pet_id: &str) -> String {
        self.cached_spritesheet_path(pet_id)
            .to_string_lossy()
            .into_owned()
    }

    /// Save a pet config to disk, skipping the write if the content is unchanged.
    pub async fn save_config(&self, pet_id: &str, config: &PetConfig) -> Result<()> {
        let path = self.cached_config_path(pet_id);
        let json = serde_json::to_string_pretty(config)?;
        if path.exists() {
            if let Ok(existing) = tokio::fs::read_to_string(&path).await {
                if existing == json {
                    return Ok(());
                }
            }
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    /// Load a pet config from disk. Returns None if the file doesn't exist.
    #[allow(dead_code)]
    pub async fn load_config(&self, pet_id: &str) -> Result<Option<PetConfig>> {
        let path = self.cached_config_path(pet_id);
        if !path.exists() {
            return Ok(None);
        }
        let json = tokio::fs::read_to_string(&path).await?;
        let config: PetConfig = serde_json::from_str(&json)?;
        Ok(Some(config))
    }

    /// List all downloaded pets by scanning the cache directory for sprite-pet.json files.
    pub async fn list_cached_pets(&self) -> Result<Vec<PetConfig>> {
        let cache_dir = &self.config.cache_dir;
        if !cache_dir.exists() {
            return Ok(Vec::new());
        }
        let mut pets = Vec::new();
        let mut entries = tokio::fs::read_dir(cache_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let config_path = entry.path().join("sprite-pet.json");
            if config_path.exists() {
                if let Ok(json) = tokio::fs::read_to_string(&config_path).await {
                    if let Ok(config) = serde_json::from_str::<PetConfig>(&json) {
                        pets.push(config);
                    }
                }
            }
        }
        Ok(pets)
    }

    /// Clear the local cache for a specific pet or all pets.
    pub async fn clear_cache(&self, pet_id: Option<&str>) -> Result<()> {
        let target = match pet_id {
            Some(id) => self.config.cache_dir.join(id),
            None => self.config.cache_dir.clone(),
        };
        if target.exists() {
            tokio::fs::remove_dir_all(&target).await?;
        }
        Ok(())
    }
}

const IMAGE_EXTENSIONS: &[&str] = &["webp", "png", "jpg", "jpeg", "gif", "bmp"];

/// Scan a directory for a cached spritesheet file with any supported image extension.
fn find_cached_spritesheet(dir: &std::path::Path) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }
    for ext in IMAGE_EXTENSIONS {
        let path = dir.join(format!("spritesheet.{ext}"));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Map an HTTP Content-Type header to a file extension.
fn extension_from_content_type(content_type: Option<&str>) -> &'static str {
    let ct = content_type.unwrap_or("").split(';').next().unwrap_or("").trim();
    match ct {
        "image/webp" => "webp",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "webp",
    }
}

/// Compute CRC32 hash of a file's contents. Returns a hex string.
pub(crate) fn file_crc32(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let hash = crc32fast::hash(&bytes);
    Ok(format!("{hash:08x}"))
}
