use crate::error::Result;
use crate::models::FrameLayout;
use image::DynamicImage;
use serde::{Deserialize, Serialize};

/// Validation configuration.
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub expected_layout: Option<FrameLayout>,
    pub require_alpha: bool,
    #[allow(dead_code)]
    pub max_atlas_bytes: u64,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            expected_layout: None,
            require_alpha: true,
            max_atlas_bytes: 10 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutcome {
    pub valid: bool,
    pub detected_layout: FrameLayout,
    pub has_alpha: bool,
    pub image_width: u32,
    pub image_height: u32,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Warning,
    Error,
}

/// Validate raw bytes before decoding (size check).
#[allow(dead_code)]
pub fn validate_bytes(bytes: &[u8], config: &ValidationConfig) -> Result<()> {
    if bytes.len() as u64 > config.max_atlas_bytes {
        return Err(crate::error::Error::Validation(vec![format!(
            "File too large: {} bytes (max {})",
            bytes.len(),
            config.max_atlas_bytes
        )]));
    }
    Ok(())
}

/// Auto-detect frame layout from image dimensions.
pub fn detect_layout(width: u32, height: u32) -> Option<FrameLayout> {
    let candidates: Vec<(u32, u32)> = vec![
        (8, 9),
        (8, 8),
        (6, 8),
        (4, 4),
        (4, 6),
        (6, 6),
        (10, 8),
        (8, 10),
    ];

    for (cols, rows) in candidates {
        if width % cols == 0 && height % rows == 0 {
            let cell_w = width / cols;
            let cell_h = height / rows;
            if cell_w >= 16 && cell_h >= 16 && cell_w <= 2048 && cell_h <= 2048 {
                return Some(FrameLayout {
                    columns: cols,
                    rows,
                    cell_width: cell_w,
                    cell_height: cell_h,
                });
            }
        }
    }
    None
}

/// Validate a decoded sprite sheet image.
pub fn validate_spritesheet(
    img: &DynamicImage,
    config: &ValidationConfig,
) -> Result<ValidationOutcome> {
    let mut issues = Vec::new();
    let (w, h) = (img.width(), img.height());
    let has_alpha = img.color().has_alpha();

    let detected = detect_layout(w, h).unwrap_or(FrameLayout {
        columns: 1,
        rows: 1,
        cell_width: w,
        cell_height: h,
    });

    if config.require_alpha && !has_alpha {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            message: "Image has no alpha channel".into(),
        });
    }

    if let Some(ref expected) = config.expected_layout {
        let expected_w = expected.columns * expected.cell_width;
        let expected_h = expected.rows * expected.cell_height;
        if w != expected_w || h != expected_h {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                message: format!(
                    "Dimension mismatch: got {}×{}, expected {}×{}",
                    w, h, expected_w, expected_h
                ),
            });
        }
    }

    let valid = !issues.iter().any(|i| i.severity == IssueSeverity::Error);

    Ok(ValidationOutcome {
        valid,
        detected_layout: detected,
        has_alpha,
        image_width: w,
        image_height: h,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_layout_standard() {
        let layout = detect_layout(1536, 1872).unwrap();
        assert_eq!(layout.columns, 8);
        assert_eq!(layout.rows, 9);
        assert_eq!(layout.cell_width, 192);
        assert_eq!(layout.cell_height, 208);
    }

    #[test]
    fn test_detect_layout_unknown() {
        assert!(detect_layout(101, 103).is_none());
    }

    #[test]
    fn test_validate_bytes_too_large() {
        let config = ValidationConfig {
            max_atlas_bytes: 10,
            ..Default::default()
        };
        assert!(validate_bytes(&[0u8; 100], &config).is_err());
    }
}
