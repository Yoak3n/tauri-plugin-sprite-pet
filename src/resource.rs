use crate::error::Result;
use crate::models::{PetConfig, PetListResponse, PetMeta};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

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
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sprite-pet");
        Self {
            provider: ResourceProvider::codex_pets(),
            cache_dir,
            request_timeout: Duration::from_secs(30),
        }
    }
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
        let dest = self.cached_spritesheet_path(pet_id);
        if dest.exists() {
            return Ok(dest);
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Prefer the provider's direct spritesheet URL over the metadata URL
        let url = self.config.provider.spritesheet_url(pet_id);

        let bytes = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        tokio::fs::write(&dest, &bytes).await?;
        Ok(dest)
    }

    /// Get the cache path for a pet's spritesheet.
    pub fn cached_spritesheet_path(&self, pet_id: &str) -> PathBuf {
        self.config.cache_dir.join(pet_id).join("spritesheet.webp")
    }

    /// Get the cache path for a pet's config file.
    pub fn cached_config_path(&self, pet_id: &str) -> PathBuf {
        self.config.cache_dir.join(pet_id).join("pet.json")
    }

    /// Get the absolute path to the cached spritesheet as a string.
    pub fn spritesheet_abs_path(&self, pet_id: &str) -> String {
        self.cached_spritesheet_path(pet_id)
            .to_string_lossy()
            .into_owned()
    }

    /// Save a pet config to disk.
    pub async fn save_config(&self, pet_id: &str, config: &PetConfig) -> Result<()> {
        let path = self.cached_config_path(pet_id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(config)?;
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

    /// List all downloaded pets by scanning the cache directory for pet.json files.
    pub async fn list_cached_pets(&self) -> Result<Vec<PetConfig>> {
        let cache_dir = &self.config.cache_dir;
        if !cache_dir.exists() {
            return Ok(Vec::new());
        }
        let mut pets = Vec::new();
        let mut entries = tokio::fs::read_dir(cache_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let config_path = entry.path().join("pet.json");
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
