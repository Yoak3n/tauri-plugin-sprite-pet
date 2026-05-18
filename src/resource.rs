use crate::error::Result;
use crate::models::{PetConfig, PetListResponse, PetMeta};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

/// Supported pet resource providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceProvider {
    /// https://codex-pets.net — full metadata API with spritesheet URL in response.
    CodexPets,
    /// https://codexpet.xyz — spritesheet served as binary at /api/pets/{id}/spritesheet.
    CodexpetXyz,
}

/// Configuration for the resource module.
#[derive(Debug, Clone)]
pub struct ResourceConfig {
    pub api_base_url: String,
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
            api_base_url: "https://codex-pets.net".into(),
            provider: ResourceProvider::CodexPets,
            cache_dir,
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// Lightweight struct for deserializing codexpet.xyz pet metadata.
#[derive(Debug, Deserialize)]
struct CodexpetPetMeta {
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
    spritesheetUrl: Option<String>,
    download_url: Option<String>,
    downloadUrl: Option<String>,
}

impl CodexpetPetMeta {
    fn into_pet_meta(self, api_base_url: &str) -> PetMeta {
        let id = self.slug.or(self.id).unwrap_or_default();
        let spritesheet_url = self
            .spritesheet_url
            .or(self.spritesheetUrl)
            .unwrap_or_else(|| {
                format!("{}/api/pets/{}/spritesheet", api_base_url, id)
            });
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
            download_url: self
                .download_url
                .or(self.downloadUrl)
                .unwrap_or_default(),
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
            self.config.api_base_url, page, page_size
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
            self.config.api_base_url,
            page,
            page_size,
            urlencoding::encode(query)
        );
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let data = resp.json::<PetListResponse>().await?;
        Ok(data)
    }

    /// Get full metadata for a single pet by ID.
    /// Handles different response formats based on the configured provider.
    pub async fn get_pet(&self, id: &str) -> Result<PetMeta> {
        match self.config.provider {
            ResourceProvider::CodexPets => self.get_pet_codex_pets(id).await,
            ResourceProvider::CodexpetXyz => self.get_pet_codexpet_xyz(id).await,
        }
    }

    /// Fetch pet metadata from codex-pets.net.
    async fn get_pet_codex_pets(&self, id: &str) -> Result<PetMeta> {
        let url = format!("{}/api/pets/{}", self.config.api_base_url, id);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let data = resp.json::<serde_json::Value>().await?;
        let pet: PetMeta = serde_json::from_value(
            data.get("pet")
                .cloned()
                .unwrap_or(data),
        )?;
        Ok(pet)
    }

    /// Fetch pet metadata from codexpet.xyz.
    /// The response has a different field layout; we normalize it into PetMeta.
    async fn get_pet_codexpet_xyz(&self, id: &str) -> Result<PetMeta> {
        let url = format!("{}/api/pets/{}", self.config.api_base_url, id);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let data = resp.json::<serde_json::Value>().await?;
        let pet_value = data
            .get("pet")
            .cloned()
            .unwrap_or(data);
        let raw: CodexpetPetMeta = serde_json::from_value(pet_value)?;
        Ok(raw.into_pet_meta(&self.config.api_base_url))
    }

    /// Download and cache a pet's sprite sheet. Returns the local path.
    /// For codexpet.xyz, uses the binary spritesheet endpoint directly.
    pub async fn fetch_spritesheet(
        &self,
        pet_id: &str,
        spritesheet_url: &str,
    ) -> Result<PathBuf> {
        let dest = self.cached_spritesheet_path(pet_id);
        if dest.exists() {
            return Ok(dest);
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // For codexpet.xyz, prefer the direct binary endpoint over the metadata URL
        let url = match self.config.provider {
            ResourceProvider::CodexpetXyz => {
                format!("{}/api/pets/{}/spritesheet", self.config.api_base_url, pet_id)
            }
            _ => spritesheet_url.to_string(),
        };

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
