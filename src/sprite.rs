use crate::error::Result;
use crate::models::{FrameLayout, FrameRect, SpriteSheet};
use image::DynamicImage;
use std::io::Cursor;
use std::path::Path;

fn build_frame_index(layout: &FrameLayout) -> Vec<Vec<FrameRect>> {
    (0..layout.rows)
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
        .collect()
}

pub fn load_spritesheet(path: &Path, layout: FrameLayout) -> Result<SpriteSheet> {
    let img = image::open(path)?;
    Ok(SpriteSheet {
        image: img,
        frames: build_frame_index(&layout),
        layout,
    })
}

#[allow(dead_code)]
pub fn load_spritesheet_from_bytes(bytes: &[u8], layout: FrameLayout) -> Result<SpriteSheet> {
    let img = image::load_from_memory(bytes)?;
    Ok(SpriteSheet {
        image: img,
        frames: build_frame_index(&layout),
        layout,
    })
}

#[allow(dead_code)]
pub fn extract_frame(sheet: &SpriteSheet, row: u32, col: u32) -> Result<DynamicImage> {
    let rect = sheet
        .frames
        .get(row as usize)
        .and_then(|r| r.get(col as usize))
        .ok_or_else(|| {
            crate::error::Error::InvalidAction(format!("Frame [{row}][{col}] out of bounds"))
        })?;
    Ok(sheet
        .image
        .crop_imm(rect.x, rect.y, rect.width, rect.height))
}

#[allow(dead_code)]
pub fn extract_frame_bytes(sheet: &SpriteSheet, row: u32, col: u32) -> Result<Vec<u8>> {
    let frame = extract_frame(sheet, row, col)?;
    let mut buf = Cursor::new(Vec::new());
    frame.write_to(&mut buf, image::ImageFormat::WebP)?;
    Ok(buf.into_inner())
}

#[allow(dead_code)]
pub fn pre_extract_all(sheet: &SpriteSheet) -> Result<Vec<Vec<Vec<u8>>>> {
    let mut all = Vec::with_capacity(sheet.layout.rows as usize);
    for r in 0..sheet.layout.rows {
        let mut row_frames = Vec::with_capacity(sheet.layout.columns as usize);
        for c in 0..sheet.layout.columns {
            row_frames.push(extract_frame_bytes(sheet, r, c)?);
        }
        all.push(row_frames);
    }
    Ok(all)
}

/// Detect the actual number of frames per row by scanning for non-transparent pixels.
/// Returns a Vec of frame counts, one per row (up to `layout.rows`).
pub fn detect_frame_counts(img: &DynamicImage, layout: &FrameLayout) -> Vec<u32> {
    let rgba = img.to_rgba8();
    let (img_w, _img_h) = rgba.dimensions();
    let mut counts = Vec::with_capacity(layout.rows as usize);

    for row in 0..layout.rows {
        let mut last_non_empty_col: u32 = 0;
        let y_start = row * layout.cell_height;
        let y_end = y_start + layout.cell_height;

        for col in 0..layout.columns {
            let x_start = col * layout.cell_width;
            if x_start >= img_w {
                break;
            }
            let x_end = (x_start + layout.cell_width).min(img_w);

            // Check if this cell has any non-transparent pixels
            let mut has_content = false;
            for y in y_start..y_end {
                for x in x_start..x_end {
                    let pixel = rgba.get_pixel(x, y);
                    if pixel[3] > 10 {
                        has_content = true;
                        break;
                    }
                }
                if has_content {
                    break;
                }
            }

            if has_content {
                last_non_empty_col = col + 1;
            }
        }

        counts.push(last_non_empty_col.max(1));
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_frame_index() {
        let layout = FrameLayout::default();
        let frames = build_frame_index(&layout);
        assert_eq!(frames.len(), 9);
        assert_eq!(frames[0].len(), 8);
        assert_eq!(frames[0][0], FrameRect { x: 0, y: 0, width: 192, height: 208 });
        assert_eq!(frames[0][1], FrameRect { x: 192, y: 0, width: 192, height: 208 });
        assert_eq!(frames[1][0], FrameRect { x: 0, y: 208, width: 192, height: 208 });
    }

    #[test]
    fn test_detect_frame_counts_all_filled() {
        // Create a 1536x1872 image (8x9 grid of 192x208 cells) with all cells filled
        use image::RgbaImage;
        let img = RgbaImage::from_fn(1536, 1872, |_, _| image::Rgba([255, 0, 0, 255]));
        let dynamic = DynamicImage::ImageRgba8(img);
        let layout = FrameLayout::default();
        let counts = detect_frame_counts(&dynamic, &layout);
        assert_eq!(counts.len(), 9);
        assert!(counts.iter().all(|&c| c == 8));
    }

    #[test]
    fn test_detect_frame_counts_partial_rows() {
        // Create a 1536x1872 image where row 0 has 6 frames, row 1 has 8 frames
        use image::RgbaImage;
        let mut img = RgbaImage::from_pixel(1536, 1872, image::Rgba([0, 0, 0, 0]));
        // Fill row 0 columns 0-5 (6 frames)
        for col in 0..6 {
            for y in 0..208u32 {
                for x in (col * 192)..((col + 1) * 192) {
                    img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
                }
            }
        }
        // Fill row 1 columns 0-7 (8 frames)
        for col in 0..8 {
            for y in 208..416u32 {
                for x in (col * 192)..((col + 1) * 192) {
                    img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
                }
            }
        }
        let dynamic = DynamicImage::ImageRgba8(img);
        let layout = FrameLayout::default();
        let counts = detect_frame_counts(&dynamic, &layout);
        assert_eq!(counts[0], 6); // row 0: 6 frames
        assert_eq!(counts[1], 8); // row 1: 8 frames
        assert_eq!(counts[2], 1); // row 2: empty, defaults to 1
    }
}
