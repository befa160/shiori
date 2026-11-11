//! Redimensionnement des pages (principe 7) : décodage plafonné, Lanczos3, puis AVIF
//! (pages) ou JPEG (miniatures, couvertures). Tout en Rust pur.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, Limits};
use serde::Deserialize;

/// Largeurs servies : une liste fermée borne le cache et empêche de faire calculer des
/// milliers de variantes. Le client prend la largeur immédiatement supérieure à son besoin.
pub const WIDTHS: [u32; 9] = [160, 320, 480, 640, 800, 1080, 1280, 1600, 2048];

/// Change dès que le rendu change (filtre, qualité) : nouvelles URL de cache, pas de vieux fichiers servis.
pub const RENDER_VERSION: u32 = 1;

/// Mémoire maximale d'un décodage : une bande de webtoon 800×16 000 en RGBA pèse 51 Mio.
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

const JPEG_QUALITY: u8 = 85;
const AVIF_QUALITY: f32 = 70.0;
/// Vitesse 1 (lent, compact) à 10 (rapide) ; sans assembleur, rav1e est lent : on privilégie la vitesse.
const AVIF_SPEED: u8 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Avif,
    Jpeg,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Avif => "avif",
            Self::Jpeg => "jpg",
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// Décode `bytes`, ramène l'image à `width` pixels de large (proportions gardées) et l'encode.
pub fn render(bytes: &[u8], width: u32, format: Format) -> Result<Vec<u8>, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?;
    let image = image.resize(width, u32::MAX, FilterType::Lanczos3);
    match format {
        Format::Jpeg => jpeg(&image),
        Format::Avif => avif(&image),
    }
}

fn jpeg(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    image
        .to_rgb8()
        .write_with_encoder(encoder)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn avif(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let rgb = image.to_rgb8();
    let pixels: Vec<ravif::RGB8> = rgb
        .as_raw()
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&[r, g, b]| ravif::RGB8::new(r, g, b))
        .collect();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    ravif::Encoder::new()
        .with_quality(AVIF_QUALITY)
        .with_speed(AVIF_SPEED)
        .encode_rgb(ravif::Img::new(pixels.as_slice(), w, h))
        .map(|encoded| encoded.avif_file)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
