use std::collections::VecDeque;
use std::path::Path;

use image::{imageops::FilterType, ImageBuffer, Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChromaMode {
    Green,
    Magenta,
}

fn color_dist_sq(a: Rgba<u8>, b: Rgba<u8>) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

fn detect_chroma_mode(corner_bg: Rgba<u8>) -> ChromaMode {
    if corner_bg[0] > 170 && corner_bg[2] > 170 && corner_bg[1] < 110 {
        ChromaMode::Magenta
    } else {
        ChromaMode::Green
    }
}

fn is_chroma_bg(px: Rgba<u8>, corner_bg: Rgba<u8>, mode: ChromaMode) -> bool {
    let close_to_corner = color_dist_sq(px, corner_bg) <= 88 * 88;
    match mode {
        ChromaMode::Green => {
            let bright_key_green = px[1] >= 175
                && (px[1] as i32 - px[0] as i32) >= 65
                && (px[1] as i32 - px[2] as i32) >= 65;
            close_to_corner || bright_key_green
        }
        ChromaMode::Magenta => {
            let bright_key_magenta = px[0] >= 195
                && px[2] >= 185
                && (px[0] as i32 - px[1] as i32) >= 65
                && (px[2] as i32 - px[1] as i32) >= 55;
            close_to_corner || bright_key_magenta
        }
    }
}

fn is_enclosed_chroma(px: Rgba<u8>, mode: ChromaMode) -> bool {
    match mode {
        ChromaMode::Green => px[1] >= 190 && px[0] <= 90 && px[2] <= 90,
        ChromaMode::Magenta => {
            px[0] >= 195
                && px[2] >= 180
                && (px[0] as i32 - px[1] as i32) >= 55
                && (px[2] as i32 - px[1] as i32) >= 45
        }
    }
}

/// Extracts an isolated pixel-art prop from a flat #00FF00 or #FF00FF chroma background,
/// tightly crops to its foreground silhouette, and scales preserving exact aspect ratio
/// so `max(width, height) <= max_dim` with a 2px transparent safety margin.
fn process_prop_tight(
    input_path: &Path,
    output_path: &Path,
    max_dim: u32,
) -> Result<RgbaImage, String> {
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
        ((corners[0][0] as u32 + corners[1][0] as u32 + corners[2][0] as u32 + corners[3][0] as u32)
            / 4) as u8,
        ((corners[0][1] as u32 + corners[1][1] as u32 + corners[2][1] as u32 + corners[3][1] as u32)
            / 4) as u8,
        ((corners[0][2] as u32 + corners[1][2] as u32 + corners[2][2] as u32 + corners[3][2] as u32)
            / 4) as u8,
        255,
    ]);
    let mode = detect_chroma_mode(corner_bg);

    let mut visited = vec![false; (w * h) as usize];
    let mut bg_mask = vec![false; (w * h) as usize];
    let mut queue = VecDeque::new();

    for x in 0..w {
        for y in [0, h - 1] {
            let idx = (y * w + x) as usize;
            if !visited[idx] && is_chroma_bg(*img.get_pixel(x, y), corner_bg, mode) {
                visited[idx] = true;
                bg_mask[idx] = true;
                queue.push_back((x, y));
            }
        }
    }
    for y in 0..h {
        for x in [0, w - 1] {
            let idx = (y * w + x) as usize;
            if !visited[idx] && is_chroma_bg(*img.get_pixel(x, y), corner_bg, mode) {
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
                    if is_chroma_bg(*img.get_pixel(nx, ny), corner_bg, mode) {
                        bg_mask[nidx] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
    }

    // Clear enclosed pure key gaps (chair slats, brass foot rails, chalk body interior, boot treads).
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let px = *img.get_pixel(x, y);
            if bg_mask[idx] || is_enclosed_chroma(px, mode) {
                bg_mask[idx] = true;
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    // Despill 2px border around transparent mask to eliminate JPEG key fringe on dark outlines.
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
                match mode {
                    ChromaMode::Green => {
                        let cap = px[0].max(px[2]);
                        if px[1] > cap {
                            px[1] = cap;
                        }
                    }
                    ChromaMode::Magenta => {
                        let cap = px[1].saturating_add(18);
                        if px[0] > cap && px[2] > cap {
                            px[0] = cap;
                            px[2] = cap;
                        }
                    }
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
        return Err(format!(
            "No foreground pixels left in {}",
            input_path.display()
        ));
    }

    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;
    let cropped = dyn_crop(&img, min_x, min_y, crop_w, crop_h);

    let usable = max_dim.saturating_sub(4).max(8);
    let scale = (usable as f32 / crop_w as f32).min(usable as f32 / crop_h as f32);
    let new_w = ((crop_w as f32 * scale).round() as u32).max(1);
    let new_h = ((crop_h as f32 * scale).round() as u32).max(1);

    let mut resized = image::imageops::resize(&cropped, new_w, new_h, FilterType::Triangle);
    for px in resized.pixels_mut() {
        if px[3] < 130 {
            *px = Rgba([0, 0, 0, 0]);
        } else {
            px[3] = 255;
        }
    }

    let out_w = new_w + 4;
    let out_h = new_h + 4;
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(out_w, out_h, Rgba([0, 0, 0, 0]));
    image::imageops::overlay(&mut canvas, &resized, 2, 2);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    canvas
        .save(output_path)
        .map_err(|e| format!("Failed saving {}: {e}", output_path.display()))?;
    println!(
        "Saved prop ({:?}, {}x{}) -> {}",
        mode,
        out_w,
        out_h,
        output_path.display()
    );
    Ok(canvas)
}

/// Converts a crimson blood pixel-art sprite into a high-luminance grayscale sprite
/// so Bevy's `BloodEcho` color multiplier tints it cleanly into Crimson, Cyan, Amber, or Violet.
fn save_supernatural_echo_variant(src: &RgbaImage, output_path: &Path) -> Result<(), String> {
    let (w, h) = src.dimensions();
    let mut out: RgbaImage = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let px = *src.get_pixel(x, y);
            if px[3] == 0 {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else {
                let peak = px[0].max(px[1]).max(px[2]) as f32 / 255.0;
                let lum = (96.0 + peak * 159.0).round().clamp(90.0, 255.0) as u8;
                out.put_pixel(x, y, Rgba([lum, lum, lum, 255]));
            }
        }
    }
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    out.save(output_path)
        .map_err(|e| format!("Failed saving {}: {e}", output_path.display()))?;
    println!(
        "Saved echo ({w}x{h}) -> {}",
        output_path.display()
    );
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

/// Extracts a chroma-keyed character/corpse sprite and centers it onto a square
/// `canvas_dim x canvas_dim` transparent RGBA canvas so Bevy square `sprite_size` renders aspect-true.
fn process_square_sprite(
    input_path: &Path,
    output_path: &Path,
    canvas_dim: u32,
) -> Result<(), String> {
    let tight = process_prop_tight(input_path, output_path, canvas_dim)?;
    let (tw, th) = tight.dimensions();
    let mut square: RgbaImage = ImageBuffer::from_pixel(canvas_dim, canvas_dim, Rgba([0, 0, 0, 0]));
    let off_x = ((canvas_dim.saturating_sub(tw)) / 2) as i64;
    let off_y = ((canvas_dim.saturating_sub(th)) / 2) as i64;
    image::imageops::overlay(&mut square, &tight, off_x, off_y);
    square
        .save(output_path)
        .map_err(|e| format!("Failed saving square sprite {}: {e}", output_path.display()))?;
    println!(
        "Saved square sprite ({canvas_dim}x{canvas_dim}) -> {}",
        output_path.display()
    );
    Ok(())
}

fn process_portrait(input_path: &Path, output_path: &Path, dim: u32) -> Result<(), String> {
    let dyn_img = image::open(input_path)
        .map_err(|e| format!("Failed to open {}: {e}", input_path.display()))?;
    let resized = image::imageops::resize(&dyn_img.to_rgba8(), dim, dim, FilterType::Lanczos3);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    resized
        .save(output_path)
        .map_err(|e| format!("Failed saving portrait {}: {e}", output_path.display()))?;
    println!("Saved portrait ({dim}x{dim}) -> {}", output_path.display());
    Ok(())
}

fn run_case03_assets(raw_dir: &Path, assets: &Path) -> Result<(), String> {
    let portraits = [
        ("sabine_portrait_1793566404653.jpg", "portraits/sabine_portrait.png"),
        ("vespera_portrait_1793566421879.jpg", "portraits/vespera_portrait.png"),
        ("corso_portrait_1793566435182.jpg", "portraits/corso_portrait.png"),
    ];
    for (src, dst) in portraits {
        process_portrait(&raw_dir.join(src), &assets.join(dst), 256)?;
    }

    let square_sprites = [
        ("sabine_raw_1793566453452.jpg", "sprites/sabine.png", 128u32),
        ("vespera_raw_1793566468562.jpg", "sprites/vespera.png", 128u32),
        ("corso_raw_1793566494794.jpg", "sprites/corso.png", 128u32),
        ("cross_corpse_raw_1793566393750.jpg", "sprites/cross_corpse.png", 160u32),
    ];
    for (src, dst, dim) in square_sprites {
        process_square_sprite(&raw_dir.join(src), &assets.join(dst), dim)?;
    }

    let props = [
        ("velvet_divan_raw_1793566406512.jpg", "props/velvet_divan.png", 208u32),
        ("absinthe_tray_raw_1793566419293.jpg", "props/absinthe_tray.png", 128u32),
        ("judicial_briefcase_raw_1793566433961.jpg", "props/judicial_briefcase.png", 128u32),
    ];
    for (src, dst, max_dim) in props {
        process_prop_tight(&raw_dir.join(src), &assets.join(dst), max_dim)?;
    }
    Ok(())
}

fn main() {
    let assets = Path::new("assets");
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--case03" {
        let raw_dir = args
            .get(2)
            .map(Path::new)
            .unwrap_or_else(|| Path::new("assets/raw/case_03"));
        if let Err(e) = run_case03_assets(raw_dir, assets) {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
        return;
    }

    let raw_dir = args
        .get(1)
        .map(Path::new)
        .unwrap_or_else(|| Path::new("assets/raw"));

    // 1. Blood & Supernatural Echo Assets (save both rich crimson PNG + multi-spectrum echo PNG)
    let blood_jobs = [
        (
            "blood_pool_1793546174167.jpg",
            "props/blood_pool.png",
            "props/echo_pool.png",
            160u32,
        ),
        (
            "blood_drag_smear_1793546189708.jpg",
            "props/blood_drag_smear.png",
            "props/echo_smear.png",
            144u32,
        ),
        (
            "blood_drops_1793546206018.jpg",
            "props/blood_drops.png",
            "props/echo_drops.png",
            96u32,
        ),
        (
            "blood_bootprint_1793546233846.jpg",
            "props/blood_bootprint.png",
            "props/echo_bootprint.png",
            96u32,
        ),
    ];

    for (src, crimson_dst, echo_dst, max_dim) in blood_jobs {
        let img = match process_prop_tight(&raw_dir.join(src), &assets.join(crimson_dst), max_dim) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("ERROR: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = save_supernatural_echo_variant(&img, &assets.join(echo_dst)) {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }

    // 2. Existing-Scene & New Scene-Setting Props
    let prop_jobs = [
        // Existing Case 01 props & clues
        ("overturned_poker_table_1793546176147.jpg", "props/overturned_poker_table.png", 192u32),
        ("mahogany_bar_1793546191076.jpg", "props/mahogany_bar.png", 256u32),
        ("velvet_curtain_1793546205166.jpg", "props/velvet_curtain.png", 280u32),
        ("crimson_rug_1793546219786.jpg", "props/crimson_rug.png", 256u32),
        ("scattered_papers_1793546235737.jpg", "props/scattered_papers.png", 96u32),
        ("alley_dumpster_1793546181140.jpg", "props/alley_dumpster.png", 192u32),
        ("wooden_crates_1793546195398.jpg", "props/wooden_crates.png", 144u32),
        ("rusted_drainpipe_1793546209901.jpg", "props/rusted_drainpipe.png", 112u32),
        ("police_cruiser_1793546234189.jpg", "props/police_cruiser.png", 224u32),
        ("streetlamp_1793546252376.jpg", "props/streetlamp.png", 192u32),
        // Extra Scene-Setting Kit for future / expanded noir scenes
        ("detective_desk_1793546180158.jpg", "props/detective_desk.png", 192u32),
        ("filing_cabinet_1793546192739.jpg", "props/filing_cabinet.png", 160u32),
        ("vintage_jukebox_1793546206859.jpg", "props/vintage_jukebox.png", 176u32),
        ("club_table_chairs_1793546223268.jpg", "props/club_table_chairs.png", 176u32),
        ("neon_lounge_sign_1793546237662.jpg", "props/neon_lounge_sign.png", 224u32),
        ("phone_booth_1793546253351.jpg", "props/phone_booth.png", 192u32),
        ("metal_trash_cans_1793546181291.jpg", "props/metal_trash_cans.png", 144u32),
        ("harbor_bollard_barrels_1793546194540.jpg", "props/harbor_bollard_barrels.png", 160u32),
        ("fire_hydrant_1793546206320.jpg", "props/fire_hydrant.png", 112u32),
        ("morgue_autopsy_table_1793546221598.jpg", "props/morgue_autopsy_table.png", 208u32),
        ("occult_candelabra_1793546237899.jpg", "props/occult_candelabra.png", 176u32),
        ("crime_chalk_markers_1793546253784.jpg", "props/crime_chalk_markers.png", 176u32),
    ];

    for (src, dst, max_dim) in prop_jobs {
        if let Err(e) = process_prop_tight(&raw_dir.join(src), &assets.join(dst), max_dim) {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
