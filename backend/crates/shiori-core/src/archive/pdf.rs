//! PDF lu comme une archive (phase 8, voir ROADMAP.md) : au lieu d'afficher la page PDF
//! avec ses marges, Shiori sert l'image incluse dans la page, comme une page de CBZ.
//!
//! Chaque page du PDF dont les images couvrent au moins la moitié de la surface donne une
//! entrée. Un JPEG droit, sans masque, est servi tel quel (ses octets lus directement dans
//! le fichier) ; toute autre image est décodée par hayro puis encodée en PNG. Les pages
//! écartées et le texte visible perdu sont des [`Issue`] du fichier (principe 4).

use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hayro_interpret::font::GlyphRun;
use hayro_interpret::hayro_syntax::object::{Array, Name, Stream};
use hayro_interpret::hayro_syntax::{Filter, LoadPdfError, Pdf};
use hayro_interpret::kurbo::{Affine, BezPath, Rect, Shape};
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageData, ImageDrawProps,
    InterpreterCache, InterpreterSettings, LumaData, RenderContent, SoftMask, TransformExt,
    interpret_page,
};
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage};

use super::{Archive, ArchiveError, Duplicate, Entry, EntryKind, MAX_ENTRY_SIZE};
use crate::integrity::{Issue, IssueCode};

/// Part minimale de la page couverte par ses images (Q56) : en dessous, c'est une page de
/// texte avec une vignette, pas une planche.
const MIN_COVERAGE: f64 = 0.5;

/// Le format se reconnaît à son contenu : l'en-tête `%PDF-` doit figurer dans le premier Kio,
/// et pas dans un zip (un CBZ dont la première entrée est un PDF stocké tel quel).
pub fn is_pdf(path: &Path) -> bool {
    let mut head = Vec::with_capacity(1024);
    File::open(path)
        .and_then(|f| f.take(1024).read_to_end(&mut head))
        .is_ok()
        && !head.starts_with(b"PK\x03\x04")
        && head.windows(5).any(|w| w == b"%PDF-")
}

pub struct PdfArchive {
    path: PathBuf,
    entries: Vec<Entry>,
    /// Une par entrée, même index.
    sources: Vec<Source>,
    issues: Vec<Issue>,
}

struct Source {
    /// Index de la page dans le PDF.
    page: usize,
    /// Octets du JPEG servi tel quel, dans le fichier ; `None` : image à décoder.
    jpeg: Option<Range<usize>>,
}

/// Ce que l'interprétation d'une page a relevé.
#[derive(Default)]
struct Probe {
    /// Adresse et longueur du contenu du fichier, pour situer les octets bruts d'une image.
    file: Range<usize>,
    page: Rect,
    images: Vec<Found>,
    visible_text: bool,
    /// Décode l'unique image au lieu de seulement la relever.
    decode: bool,
    decoded: Option<Result<DynamicImage, String>>,
}

struct Found {
    coverage: f64,
    jpeg: Option<Range<usize>>,
    raw_crc: u32,
    raw_len: u64,
}

impl Probe {
    fn found(&self, image: &Image<'_, '_>, props: &ImageDrawProps<'_>) -> Found {
        let pixels = Rect::new(0.0, 0.0, image.width().into(), image.height().into());
        let placed = (props.transform * pixels.to_path(0.1)).bounding_box();
        let coverage = placed.intersect(self.page).area() / self.page.area().max(1.0);
        let (stream, raster) = match image {
            Image::Raster(r) => (r.stream(), true),
            Image::Stencil(s) => (s.stream(), false),
        };
        let raw = stream.raw_data();
        let at = raw.as_ptr() as usize;
        let in_file = at >= self.file.start && at + raw.len() <= self.file.end;
        // Octets empruntés au fichier (pas déchiffrés) : on peut les relire tels quels.
        let jpeg = (raster
            && in_file
            && props.soft_mask.is_none()
            && orientation(props.transform) == Orientation::Upright
            && plain_jpeg(stream))
        .then(|| at - self.file.start..at - self.file.start + raw.len());
        Found {
            coverage,
            jpeg,
            raw_crc: crc32fast::hash(&raw),
            raw_len: raw.len() as u64,
        }
    }
}

/// Un JPEG que n'importe quel décodeur affiche comme le PDF : seul filtre `DCTDecode`, gris
/// ou RVB, ni masque ni tableau `Decode`.
fn plain_jpeg(stream: &Stream<'_>) -> bool {
    let dict = stream.dict();
    let gray_or_rgb = match dict.get::<Name<'_>>(b"ColorSpace") {
        Some(n) => matches!(n.as_ref(), b"DeviceRGB" | b"DeviceGray"),
        None => dict.get::<Array<'_>>(b"ColorSpace").is_some_and(|a| {
            let mut it = a.flex_iter();
            it.next::<Name<'_>>()
                .is_some_and(|n| n.as_ref() == b"ICCBased")
                && it
                    .next::<Stream<'_>>()
                    .and_then(|icc| icc.dict().get::<u32>(b"N"))
                    .is_some_and(|n| n == 1 || n == 3)
        }),
    };
    stream.filters().as_slice() == [Filter::DctDecode]
        && gray_or_rgb
        && !["SMask", "Mask", "Decode", "ImageMask"]
            .iter()
            .any(|k| dict.contains_key(k))
}

impl<'a> Device<'a> for Probe {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph_run(&mut self, _: &GlyphRun<'_, 'a>, _: DrawProps<'a>, mode: &DrawMode) {
        self.visible_text |= !matches!(mode, DrawMode::Invisible);
    }

    fn draw_image(&mut self, image: Image<'a, '_>, props: ImageDrawProps<'a>) {
        let found = self.found(&image, &props);
        if self.decode && self.images.is_empty() {
            self.decoded = Some(decode(&image).map(|img| orient(img, props.transform)));
        }
        self.images.push(found);
    }
}

/// Pixels de l'image, aplatis sur fond blanc s'ils ont une transparence.
fn decode(image: &Image<'_, '_>) -> Result<DynamicImage, String> {
    // Les dimensions viennent du fichier : un PDF piégé annoncerait des Go de pixels.
    let pixels = u64::from(image.width()) * u64::from(image.height());
    if pixels.saturating_mul(4) > MAX_ENTRY_SIZE {
        return Err(format!(
            "image trop grande ({} × {} pixels)",
            image.width(),
            image.height()
        ));
    }
    let mut out = None;
    match image {
        Image::Raster(r) => r.with_rgba(|data, alpha| out = to_image(data, alpha), None),
        // Masque 1 bit (scan noir et blanc) : peint en noir sur blanc.
        // shortcut: la couleur de peinture du masque est ignorée (toujours noire).
        Image::Stencil(s) => s.with_stencil(
            |mask, _| {
                out = GrayImage::from_raw(mask.width, mask.height, mask.data).map(|mut g| {
                    g.pixels_mut().for_each(|p| p.0[0] = 255 - p.0[0]);
                    DynamicImage::ImageLuma8(g)
                })
            },
            None,
        ),
    }
    out.ok_or_else(|| "image du PDF illisible".to_owned())
}

fn to_image(data: ImageData, alpha: Option<LumaData>) -> Option<DynamicImage> {
    let alpha = alpha.filter(|a| a.width == data.width() && a.height == data.height());
    let over_white =
        |c: u8, a: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
    match data {
        ImageData::Rgb(d) => {
            let mut px = d.data;
            if let Some(a) = &alpha {
                for (rgb, &a) in px.as_chunks_mut::<3>().0.iter_mut().zip(&a.data) {
                    rgb.iter_mut().for_each(|c| *c = over_white(*c, a));
                }
            }
            RgbImage::from_raw(d.width, d.height, px).map(DynamicImage::ImageRgb8)
        }
        ImageData::Luma(d) => {
            let mut px = d.data;
            if let Some(a) = &alpha {
                px.iter_mut()
                    .zip(&a.data)
                    .for_each(|(c, &a)| *c = over_white(*c, a));
            }
            GrayImage::from_raw(d.width, d.height, px).map(DynamicImage::ImageLuma8)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Upright,
    FlipH,
    FlipV,
    Rotate180,
    Rotate90,
    Rotate270,
    Transpose,
    Transverse,
}

/// Orientation à l'écran d'une image dessinée par `transform` (pixels de l'image vers la
/// page, y vers le bas), arrondie au quart de tour le plus proche.
fn orientation(transform: Affine) -> Orientation {
    let [a, b, c, d, _, _] = transform.as_coeffs();
    if a.abs() + d.abs() >= b.abs() + c.abs() {
        match (a > 0.0, d > 0.0) {
            (true, true) => Orientation::Upright,
            (false, true) => Orientation::FlipH,
            (true, false) => Orientation::FlipV,
            (false, false) => Orientation::Rotate180,
        }
    } else {
        // L'axe x de l'image devient vertical à l'écran.
        match (b > 0.0, c > 0.0) {
            (true, false) => Orientation::Rotate90,
            (false, true) => Orientation::Rotate270,
            (true, true) => Orientation::Transpose,
            (false, false) => Orientation::Transverse,
        }
    }
}

fn orient(img: DynamicImage, transform: Affine) -> DynamicImage {
    match orientation(transform) {
        Orientation::Upright => img,
        Orientation::FlipH => img.fliph(),
        Orientation::FlipV => img.flipv(),
        Orientation::Rotate180 => img.rotate180(),
        Orientation::Rotate90 => img.rotate90(),
        Orientation::Rotate270 => img.rotate270(),
        Orientation::Transpose => img.rotate90().fliph(),
        Orientation::Transverse => img.rotate270().fliph(),
    }
}

fn load(path: &Path) -> Result<(Pdf, Range<usize>), ArchiveError> {
    // shortcut: le PDF est lu en entier en mémoire (hayro le demande, et `unsafe_code` interdit
    // de le projeter) ; un PDF de plusieurs Gio en demanderait autant à l'ouverture.
    let bytes = Arc::new(std::fs::read(path).map_err(ArchiveError::Io)?);
    // hayro reconstruit la table des objets d'un fichier coupé et l'ouvrirait avec des pages
    // en moins : un PDF complet finit toujours par `%%EOF` (principe 4).
    let tail = &bytes[bytes.len().saturating_sub(1024)..];
    if !tail.windows(5).any(|w| w == b"%%EOF") {
        return Err(ArchiveError::Truncated("fin du PDF absente (%%EOF)".into()));
    }
    let file = bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len();
    let pdf = Pdf::new(bytes).map_err(|e| match e {
        LoadPdfError::Decryption(_) => ArchiveError::Encrypted,
        LoadPdfError::Invalid => ArchiveError::Unreadable("PDF illisible".into()),
    })?;
    Ok((pdf, file))
}

/// Interprète une page (son contenu seul, sans les annotations) et relève ses images.
fn probe(pdf: &Pdf, index: usize, file: Range<usize>, decode: bool) -> Probe {
    let page = &pdf.pages()[index];
    let (w, h) = page.render_dimensions();
    let bbox = Rect::new(0.0, 0.0, w.into(), h.into());
    let settings = InterpreterSettings {
        render_content: RenderContent::PAGE,
        ..InterpreterSettings::default()
    };
    let cache = InterpreterCache::new();
    let mut ctx = Context::new(
        page.initial_transform(true).to_kurbo(),
        bbox,
        &cache,
        pdf.xref(),
        settings,
    );
    let mut probe = Probe {
        file,
        page: bbox,
        decode,
        ..Probe::default()
    };
    interpret_page(page, &mut ctx, &mut probe);
    probe
}

impl PdfArchive {
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        let (pdf, file) = load(path)?;
        let mut archive = Self {
            path: path.to_path_buf(),
            entries: Vec::new(),
            sources: Vec::new(),
            issues: Vec::new(),
        };
        for page in 0..pdf.pages().len() {
            let probe = probe(&pdf, page, file.clone(), false);
            let label = format!("page {}", page + 1);
            let coverage: f64 = probe.images.iter().map(|i| i.coverage).sum();
            if coverage < MIN_COVERAGE {
                archive.issue(
                    IssueCode::PdfPageWithoutImage,
                    &label,
                    "page sans image (ou presque), ignorée",
                );
                continue;
            }
            if probe.visible_text {
                archive.issue(
                    IssueCode::PdfTextIgnored,
                    &label,
                    "texte visible de la page ignoré : seule l'image est affichée",
                );
            }
            let first = &probe.images[0];
            let jpeg = first.jpeg.clone().filter(|_| probe.images.len() == 1);
            let ext = if jpeg.is_some() { "jpg" } else { "png" };
            let name = format!("{:04}.{ext}", page + 1);
            archive.entries.push(Entry {
                index: archive.entries.len(),
                name_raw: name.clone().into_bytes(),
                name,
                kind: EntryKind::File,
                compressed_size: first.raw_len,
                size: first.raw_len,
                crc32: first.raw_crc,
                compression: "pdf".into(),
                compression_supported: true,
                encrypted: false,
            });
            archive.sources.push(Source { page, jpeg });
        }
        Ok(archive)
    }

    fn issue(&mut self, code: IssueCode, entry: &str, message: &str) {
        self.issues.push(Issue {
            code,
            severity: code.severity(),
            entry: Some(entry.to_owned()),
            message: message.to_owned(),
        });
    }

    /// Contenu d'une page ; `loaded` : le PDF déjà chargé, sinon chargé ici s'il faut décoder.
    fn page(
        &self,
        index: usize,
        loaded: &mut Option<(Pdf, Range<usize>)>,
    ) -> Result<Vec<u8>, ArchiveError> {
        let source = self
            .sources
            .get(index)
            .ok_or(ArchiveError::NoSuchEntry(index))?;
        let crc = self.entries[index].crc32;
        if let Some(range) = &source.jpeg {
            let mut file = File::open(&self.path).map_err(ArchiveError::Io)?;
            file.seek(SeekFrom::Start(range.start as u64))
                .map_err(ArchiveError::Io)?;
            let mut bytes = vec![0; range.len()];
            file.read_exact(&mut bytes)
                .map_err(ArchiveError::from_read_error)?;
            if crc32fast::hash(&bytes) != crc {
                return Err(ArchiveError::CrcMismatch);
            }
            return Ok(bytes);
        }
        if loaded.is_none() {
            *loaded = Some(load(&self.path)?);
        }
        let (pdf, file) = loaded.as_ref().expect("chargé juste au-dessus");
        let probe = probe(pdf, source.page, file.clone(), true);
        if probe.images.first().map(|i| i.raw_crc) != Some(crc) {
            return Err(ArchiveError::CrcMismatch);
        }
        if probe.images.len() > 1 {
            // Recomposition des pages en plusieurs images : lot 8.2.
            return Err(ArchiveError::ImageUndecodable(
                "page faite de plusieurs images, pas encore recomposée".into(),
            ));
        }
        let img = probe
            .decoded
            .unwrap_or_else(|| Err("image du PDF illisible".into()))
            .map_err(ArchiveError::ImageUndecodable)?;
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png)
            .map_err(|e| ArchiveError::ImageUndecodable(e.to_string()))?;
        Ok(out.into_inner())
    }
}

impl Archive for PdfArchive {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn duplicates(&self) -> Result<Vec<Duplicate>, ArchiveError> {
        Ok(Vec::new())
    }

    fn open_entry(&mut self, index: usize) -> Result<Box<dyn io::Read + '_>, ArchiveError> {
        Ok(Box::new(Cursor::new(self.page(index, &mut None)?)))
    }

    /// Le PDF n'est chargé qu'une fois pour toutes les pages à décoder (scan), au lieu d'une
    /// fois par page.
    fn read_each(
        &mut self,
        indices: &[usize],
        visit: &mut dyn FnMut(usize, Result<Vec<u8>, ArchiveError>),
    ) {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let mut loaded = None;
        for i in sorted {
            visit(i, self.page(i, &mut loaded));
        }
    }

    fn issues(&self) -> &[Issue] {
        &self.issues
    }
}

#[cfg(test)]
mod tests;
