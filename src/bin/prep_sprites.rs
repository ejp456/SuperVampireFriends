use std::collections::VecDeque;
use std::path::Path;

use image::{imageops::FilterType, ImageBuffer, Rgba, RgbaImage};

fn color_dist_sq(a: Rgba<u8>, b: Rgba<u8>) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

fn is_chroma_green(px: Rgba<u8>, corner_bg: Rgba<u8>) -> bool {
    let close_to_corner = color_dist_sq(px, corner_bg) <= 85 * 85;
    let bright_key_green =
        px[1] >= 180 && (px[1] as i32 - px[0] as i32) >= 70 && (px[1] as i32 - px[2] as i32) >= 70;
    close_to_corner || bright_key_green
}

fn process_sprite(input_path: &Path, output_path: &Path, target_canvas: u32) -> Result<(), String> {
    let dyn_img = image::open(input_path)
        .map_err(|e| format!("Failed to open {}: {e}", input_path.display()))?;
    let mut img: RgbaImage = dyn_img.to_rgba8();
    let (w, h) = img.dimensions();

    let corners = [
        *img.get_pixel(0, 0),
        *img.get_pixel(w - 1, 0),
        *img.get_pixel(0, h - 1),
        *img.get_pixel(w - 1, h - 1),
    ];
    let corner_bg = Rgba([
        ((corners[0][0] as u32 + corners[1][0] as u32 + corners[2][0] as u32 + corners[3][0] as u32) / 4) as u8,
        ((corners[0][1] as u32 + corners[1][1] as u32 + corners[2][1] as u32 + corners[3][1] as u32) / 4) as u8,
        ((corners[0][2] as u32 + corners[1][2] as u32 + corners[2][2] as u32 + corners[3][2] as u32) / 4) as u8,
        255,
    ]);

    let mut visited = vec![false; (w * h) as usize];
    let mut bg_mask = vec![false; (w * h) as usize];
    let mut queue = VecDeque::new();

    for x in 0..w {
        for y in [0, h - 1] {
            let idx = (y * w + x) as usize;
            if !visited[idx] && is_chroma_green(*img.get_pixel(x, y), corner_bg) {
                visited[idx] = true;
                bg_mask[idx] = true;
                queue.push_back((x, y));
            }
        }
    }
    for y in 0..h {
        for x in [0, w - 1] {
            let idx = (y * w + x) as usize;
            if !visited[idx] && is_chroma_green(*img.get_pixel(x, y), corner_bg) {
                visited[idx] = true;
                bg_mask[idx] = true;
                queue.push_back((x, y));
            }
        }
    }

    while let Some((cx, cy)) = queue.pop_front() {
        let neighbors = [
            (cx.wrapping_sub(1), cy),
            (cx + 1, cy),
            (cx, cy.wrapping_sub(1)),
            (cx, cy + 1),
        ];
        for (nx, ny) in neighbors {
            if nx < w && ny < h {
                let nidx = (ny * w + nx) as usize;
                if !visited[nidx] {
                    visited[nidx] = true;
                    if is_chroma_green(*img.get_pixel(nx, ny), corner_bg) {
                        bg_mask[nidx] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
    }

    // Clear enclosed bright #00FF00 gaps (between arms/body/legs) while preserving dark emerald gown.
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let px = *img.get_pixel(x, y);
            let enclosed_neon = px[1] >= 195 && px[0] <= 85 && px[2] <= 85;
            if bg_mask[idx] || enclosed_neon {
                bg_mask[idx] = true;
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    // Despill 2px border around transparent mask to eliminate JPEG green fringe on dark outlines.
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if bg_mask[idx] {
                continue;
            }
            let mut near_bg = false;
            'ring: for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                        if bg_mask[(ny as u32 * w + nx as u32) as usize] {
                            near_bg = true;
                            break 'ring;
                        }
                    }
                }
            }
            if near_bg {
                let mut px = *img.get_pixel(x, y);
                let cap = px[0].max(px[2]);
                if px[1] > cap {
                    px[1] = cap;
                }
                img.put_pixel(x, y, px);
            }
        }
    }

    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > 20 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if min_x > max_x || min_y > max_y {
        return Err(format!("No foreground pixels left in {}", input_path.display()));
    }

    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;
    let cropped = dyn_crop(&img, min_x, min_y, crop_w, crop_h);

    let usable = target_canvas - 8;
    let scale = (usable as f32 / crop_w as f32).min(usable as f32 / crop_h as f32);
    let new_w = ((crop_w as f32 * scale).round() as u32).max(1);
    let new_h = ((crop_h as f32 * scale).round() as u32).max(1);

    let mut resized = image::imageops::resize(&cropped, new_w, new_h, FilterType::Triangle);
    // Crisp binary alpha for clean pixel-art silhouettes
    for px in resized.pixels_mut() {
        if px[3] < 135 {
            *px = Rgba([0, 0, 0, 0]);
        } else {
            px[3] = 255;
        }
    }

    let mut canvas: RgbaImage =
        ImageBuffer::from_pixel(target_canvas, target_canvas, Rgba([0, 0, 0, 0]));
    let off_x = (target_canvas - new_w) / 2;
    let off_y = (target_canvas - new_h) / 2;
    image::imageops::overlay(&mut canvas, &resized, off_x as i64, off_y as i64);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    canvas
        .save(output_path)
        .map_err(|e| format!("Failed saving {}: {e}", output_path.display()))?;
    println!("Saved sprite -> {}", output_path.display());
    Ok(())
}

fn process_portrait(input_path: &Path, output_path: &Path, size: u32) -> Result<(), String> {
    let dyn_img = image::open(input_path)
        .map_err(|e| format!("Failed to open {}: {e}", input_path.display()))?;
    let resized = image::imageops::resize(&dyn_img.to_rgba8(), size, size, FilterType::Lanczos3);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    resized
        .save(output_path)
        .map_err(|e| format!("Failed saving {}: {e}", output_path.display()))?;
    println!("Saved portrait -> {}", output_path.display());
    Ok(())
}

fn dyn_crop(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let mut out = ImageBuffer::new(w, h);
    for row in 0..h {
        for col in 0..w {
            out.put_pixel(col, row, *img.get_pixel(x + col, y + row));
        }
    }
    out
}

fn main() {
    let brain = Path::new(r"C:\Users\etaip\.gemini\antigravity\brain\83366195-8834-4459-9701-076d152f22be");
    let assets = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"));

    let sprite_jobs = [
        ("vale_sprite_raw_1793475228150.jpg", "sprites/vale.png", 128u32),
        ("clara_sprite_raw_1793475250413.jpg", "sprites/clara.png", 128u32),
        ("miller_sprite_raw_1793475267395.jpg", "sprites/miller.png", 128u32),
        ("vance_corpse_raw_1793475283659.jpg", "sprites/vance_corpse.png", 160u32),
    ];

    for (src, dst, sz) in sprite_jobs {
        if let Err(e) = process_sprite(&brain.join(src), &assets.join(dst), sz) {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }

    let portrait_jobs = [
        ("vale_portrait_1793475236165.jpg", "portraits/vale_portrait.png", 256u32),
        ("clara_portrait_1793475250226.jpg", "portraits/clara_portrait.png", 256u32),
        ("miller_portrait_1793475265062.jpg", "portraits/miller_portrait.png", 256u32),
    ];

    for (src, dst, sz) in portrait_jobs {
        if let Err(e) = process_portrait(&brain.join(src), &assets.join(dst), sz) {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
