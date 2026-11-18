//! PDF lu comme une archive (phase 8, voir ROADMAP.md) : au lieu d'afficher la page PDF
//! avec ses marges, Shiori sert l'image incluse dans la page, comme une page de CBZ.
//!
//! Chaque page du PDF dont les images couvrent au moins la moitié de la surface donne une
//! entrée. Un JPEG droit, sans masque, est servi tel quel (ses octets lus directement dans
//! le fichier) ; toute autre page est décodée par hayro, recomposée si elle est faite de
//! plusieurs images ([`compose`]), puis encodée en PNG. Les pages écartées et le texte
//! visible perdu sont des [`Issue`] du fichier (principe 4).

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
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage, Rgba, RgbaImage, imageops};

use super::{Archive, ArchiveError, Duplicate, Entry, EntryKind, MAX_ENTRY_SIZE};
use crate::integrity::{Issue, IssueCode};
use crate::metadata::{self, LocalMetadata};

/// Part minimale de la page couverte par ses images (Q56) : en dessous, c'est une page de
/// texte avec une vignette, pas une planche.
const MIN_COVERAGE: f64 = 0.5;

/// En dessous d'un millième de la page, un morceau (pixel de suivi, image dessinée à taille
/// nulle) n'entre pas dans la recomposition : sa densité ferait une page démesurée.
const MIN_PIECE: f64 = 0.001;

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
    metadata: Option<LocalMetadata>,
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
    /// Décode aussi chaque image, avec sa place sur la page.
    decode: bool,
    /// Les morceaux décodés ([`MIN_PIECE`] au moins), dans l'ordre de dessin.
    decoded: Vec<Result<(RgbaImage, Rect), String>>,
    /// Pixels déjà décodés pour cette page : toutes ses images sont en mémoire à la fois.
    decoded_pixels: u64,
}

struct Found {
    coverage: f64,
    jpeg: Option<Range<usize>>,
    raw_crc: u32,
    raw_len: u64,
}

/// Rectangle occupé par l'image sur la page (points, y vers le bas).
fn placed(image: &Image<'_, '_>, transform: Affine) -> Rect {
    let pixels = Rect::new(0.0, 0.0, image.width().into(), image.height().into());
    (transform * pixels.to_path(0.1)).bounding_box()
}

impl Probe {
    /// Les images qui comptent : les miettes (pixel de suivi) sont écartées.
    fn pieces(&self) -> impl Iterator<Item = &Found> {
        self.images.iter().filter(|i| i.coverage >= MIN_PIECE)
    }

    fn found(&self, image: &Image<'_, '_>, props: &ImageDrawProps<'_>) -> Found {
        let placed = placed(image, props.transform);
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
        if self.decode && found.coverage >= MIN_PIECE {
            self.decoded_pixels += u64::from(image.width()) * u64::from(image.height());
            let img = if self.decoded_pixels.saturating_mul(4) > MAX_ENTRY_SIZE {
                Err("images de la page trop grandes, ensemble".to_owned())
            } else {
                decode(&image).map(|img| {
                    (
                        orient(img, props.transform),
                        placed(&image, props.transform),
                    )
                })
            };
            self.decoded.push(img);
        }
        self.images.push(found);
    }
}

/// Pixels de l'image, avec sa transparence (`SMask`, masque 1 bit).
fn decode(image: &Image<'_, '_>) -> Result<RgbaImage, String> {
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
        // Masque 1 bit (scan noir et blanc) : du noir là où il peint, transparent ailleurs.
        // shortcut: la couleur de peinture du masque est ignorée (toujours noire).
        Image::Stencil(s) => s.with_stencil(
            |mask, _| {
                out = GrayImage::from_raw(mask.width, mask.height, mask.data).map(|g| {
                    RgbaImage::from_fn(g.width(), g.height(), |x, y| {
                        Rgba([0, 0, 0, g.get_pixel(x, y).0[0]])
                    })
                })
            },
            None,
        ),
    }
    out.ok_or_else(|| "image du PDF illisible".to_owned())
}

fn to_image(data: ImageData, alpha: Option<LumaData>) -> Option<RgbaImage> {
    let (w, h) = (data.width(), data.height());
    let mut img = match data {
        ImageData::Rgb(d) => DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, d.data)?),
        ImageData::Luma(d) => DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, d.data)?),
    }
    .to_rgba8();
    if let Some(alpha) = alpha.filter(|a| (a.width, a.height) == (w, h)) {
        for (p, a) in img.pixels_mut().zip(alpha.data) {
            p.0[3] = a;
        }
    }
    Some(img)
}

/// Recompose une page à partir de ses images (bandes, tuiles, calques), chacune à sa place,
/// sur fond blanc. La page est rognée aux images : pas de marge. Une image seule garde ses
/// pixels, même dessinée déformée (une planche étirée pour remplir la page, par Calibre).
fn compose(images: Vec<(RgbaImage, Rect)>) -> Result<RgbaImage, String> {
    let blank = |w, h| RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
    if let [(img, _)] = images.as_slice() {
        let mut page = blank(img.width(), img.height());
        imageops::overlay(&mut page, img, 0, 0);
        return Ok(page);
    }
    let union = images
        .iter()
        .map(|(_, r)| *r)
        .reduce(|a, b| a.union(b))
        .ok_or("page sans image")?;
    // Densité de l'image la plus fine, sur chaque axe : aucune ne perd de pixels.
    let density = |px: fn(&RgbaImage) -> u32, len: fn(&Rect) -> f64| {
        images
            .iter()
            .map(|(img, r)| f64::from(px(img)) / len(r).max(f64::EPSILON))
            .fold(0.0, f64::max)
    };
    let (sx, sy) = (
        density(|i| i.width(), Rect::width),
        density(|i| i.height(), Rect::height),
    );
    let size = |len: f64, scale: f64| (len * scale).round().max(1.0);
    let (w, h) = (size(union.width(), sx), size(union.height(), sy));
    if w * h * 4.0 > MAX_ENTRY_SIZE as f64 {
        return Err(format!("page recomposée trop grande ({w} × {h} pixels)"));
    }
    let mut page = blank(w as u32, h as u32);
    // Les deux bords de chaque morceau sont arrondis, pas sa position et sa taille : deux
    // bandes voisines partagent exactement le même bord, sans filet blanc entre elles.
    let edges = |a: f64, b: f64, origin: f64, scale: f64| {
        let (lo, hi) = (
            ((a - origin) * scale).round(),
            ((b - origin) * scale).round(),
        );
        (lo as i64, (hi - lo).max(1.0) as u32)
    };
    for (img, r) in images {
        let (x, tw) = edges(r.x0, r.x1, union.x0, sx);
        let (y, th) = edges(r.y0, r.y1, union.y0, sy);
        let img = if img.dimensions() == (tw, th) {
            img
        } else {
            imageops::resize(&img, tw, th, imageops::FilterType::Triangle)
        };
        imageops::overlay(&mut page, &img, x, y);
    }
    Ok(page)
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

fn orient(img: RgbaImage, transform: Affine) -> RgbaImage {
    use imageops::{flip_horizontal, flip_vertical, rotate90, rotate180, rotate270};
    match orientation(transform) {
        Orientation::Upright => img,
        Orientation::FlipH => flip_horizontal(&img),
        Orientation::FlipV => flip_vertical(&img),
        Orientation::Rotate180 => rotate180(&img),
        Orientation::Rotate90 => rotate90(&img),
        Orientation::Rotate270 => rotate270(&img),
        Orientation::Transpose => flip_horizontal(&rotate90(&img)),
        Orientation::Transverse => flip_horizontal(&rotate270(&img)),
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
            metadata: {
                let info = pdf.metadata();
                metadata::pdf_info(
                    info.author.as_deref(),
                    info.subject.as_deref(),
                    info.keywords.as_deref(),
                )
            },
        };
        for page in 0..pdf.pages().len() {
            let probe = probe(&pdf, page, file.clone(), false);
            let label = format!("page {}", page + 1);
            let coverage: f64 = probe.images.iter().map(|i| i.coverage).sum();
            let pieces: Vec<&Found> = probe.pieces().collect();
            if coverage < MIN_COVERAGE || pieces.is_empty() {
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
            let first = pieces[0];
            let jpeg = first.jpeg.clone().filter(|_| pieces.len() == 1);
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
        if probe.pieces().next().map(|i| i.raw_crc) != Some(crc) {
            return Err(ArchiveError::CrcMismatch);
        }
        let images = probe
            .decoded
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .and_then(compose)
            .map_err(ArchiveError::ImageUndecodable)?;
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(images)
            .to_rgb8()
            .write_to(&mut out, ImageFormat::Png)
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

    fn metadata(&self) -> Option<LocalMetadata> {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests;
