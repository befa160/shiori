//! Fixtures PDF (phase 8), écrites avec `pdf-writer` : chaque page est une liste d'images
//! placées sur une page de 600 × 800 points, avec éventuellement du texte.

use std::path::{Path, PathBuf};

use pdf_writer::types::TextRenderingMode;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};

use crate::jpeg;

pub const PAGE_WIDTH: f32 = 600.0;
pub const PAGE_HEIGHT: f32 = 800.0;

/// Espace de couleur d'une image de fixture.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Space {
    DeviceRgb,
    /// RVB décrit par un profil ICC (`ICCBased`, `N 3`).
    IccRgb,
    /// Masque 1 bit (`ImageMask`, scan noir et blanc) : chaque pixel est peint en noir ; `rgb`
    /// et `jpeg` sont ignorés.
    Mask,
}

#[derive(Clone)]
pub struct PdfImage {
    pub width: u32,
    pub height: u32,
    pub rgb: [u8; 3],
    /// JPEG (`DCTDecode`) ; sinon pixels RVB bruts, sans filtre.
    pub jpeg: bool,
    /// Position sur la page, en points : x, y (depuis le bas), largeur, hauteur.
    pub at: [f32; 4],
    pub space: Space,
    /// Dimensions annoncées par le dictionnaire, si elles diffèrent des pixels écrits (image
    /// piégée qui annonce des Go).
    pub declared: Option<(u32, u32)>,
    /// Opacité uniforme donnée par un `SMask` (0 : transparent).
    pub alpha: Option<u8>,
}

impl PdfImage {
    /// Image qui couvre toute la page.
    pub fn full(width: u32, height: u32, rgb: [u8; 3], jpeg: bool) -> Self {
        Self {
            width,
            height,
            rgb,
            jpeg,
            at: [0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT],
            space: Space::DeviceRgb,
            declared: None,
            alpha: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct PdfPage {
    pub images: Vec<PdfImage>,
    /// Texte écrit en Helvetica (police non incluse) ; `true` : visible, `false` : couche OCR.
    pub text: Option<bool>,
    /// `/Rotate` de la page, en degrés.
    pub rotate: i32,
}

impl PdfPage {
    pub fn image(image: PdfImage) -> Self {
        Self {
            images: vec![image],
            ..Self::default()
        }
    }
}

/// Écrit un PDF dans `dir`.
pub fn write_pdf(dir: &Path, name: &str, pages: &[PdfPage]) -> PathBuf {
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut id = || {
        next += 1;
        Ref::new(next)
    };
    let catalog = Ref::new(1);
    let tree = id();
    let font = id();
    let page_ids: Vec<Ref> = pages.iter().map(|_| id()).collect();
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree)
        .kids(page_ids.iter().copied())
        .count(pages.len() as i32);
    pdf.type1_font(font).base_font(Name(b"Helvetica"));

    for (page, &page_id) in pages.iter().zip(&page_ids) {
        let content_id = id();
        let image_ids: Vec<Ref> = page.images.iter().map(|_| id()).collect();
        let names: Vec<String> = (0..page.images.len()).map(|i| format!("Im{i}")).collect();

        let mut content = Content::new();
        for (image, name) in page.images.iter().zip(&names) {
            let [x, y, w, h] = image.at;
            content.save_state();
            content.transform([w, 0.0, 0.0, h, x, y]);
            content.x_object(Name(name.as_bytes()));
            content.restore_state();
        }
        if let Some(visible) = page.text {
            content.begin_text();
            content.set_font(Name(b"F1"), 24.0);
            if !visible {
                content.set_text_rendering_mode(TextRenderingMode::Invisible);
            }
            content.next_line(50.0, 50.0);
            content.show(Str(b"Credits"));
            content.end_text();
        }
        pdf.stream(content_id, &content.finish());

        for (image, &image_id) in page.images.iter().zip(&image_ids) {
            let icc = id();
            let smask = id();
            let mask = image.space == Space::Mask;
            let data = if mask {
                // Échantillon 0 = peint : des octets nuls peignent toute l'image.
                vec![0; (image.width.div_ceil(8) * image.height) as usize]
            } else if image.jpeg {
                jpeg(image.width, image.height, image.rgb)
            } else {
                image.rgb.repeat((image.width * image.height) as usize)
            };
            let (width, height) = image.declared.unwrap_or((image.width, image.height));
            let mut xobject = pdf.image_xobject(image_id, &data);
            if image.jpeg && !mask {
                xobject.filter(Filter::DctDecode);
            }
            xobject.width(width as i32);
            xobject.height(height as i32);
            match image.space {
                Space::DeviceRgb => xobject.color_space().device_rgb(),
                Space::IccRgb => xobject.color_space().icc_based(icc),
                Space::Mask => {
                    xobject.image_mask(true);
                }
            }
            xobject.bits_per_component(if mask { 1 } else { 8 });
            if image.alpha.is_some() {
                xobject.s_mask(smask);
            }
            xobject.finish();
            if let Some(alpha) = image.alpha {
                let data = vec![alpha; (image.width * image.height) as usize];
                let mut s = pdf.image_xobject(smask, &data);
                s.width(image.width as i32).height(image.height as i32);
                s.color_space().device_gray();
                s.bits_per_component(8);
            }
            if image.space == Space::IccRgb {
                pdf.icc_profile(icc, b"profil de test").n(3);
            }
        }

        let mut p = pdf.page(page_id);
        p.media_box(Rect::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT));
        p.parent(tree);
        p.contents(content_id);
        if page.rotate != 0 {
            p.rotate(page.rotate);
        }
        let mut resources = p.resources();
        resources.fonts().pair(Name(b"F1"), font);
        let mut xobjects = resources.x_objects();
        for (name, &image_id) in names.iter().zip(&image_ids) {
            xobjects.pair(Name(name.as_bytes()), image_id);
        }
        xobjects.finish();
        resources.finish();
        p.finish();
    }
    let path = dir.join(name);
    std::fs::write(&path, pdf.finish()).unwrap();
    path
}

/// PDF chiffré (gestionnaire `Standard`, révision 2) dont le mot de passe vide ne convient pas.
pub fn encrypted_pdf(dir: &Path) -> PathBuf {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] >>".to_owned(),
        format!(
            "<< /Filter /Standard /V 1 /R 2 /P -4 /O <{}> /U <{}> >>",
            "11".repeat(32),
            "22".repeat(32)
        ),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Encrypt 4 0 R /ID [<{id}> <{id}>] >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1,
            id = "33".repeat(16)
        )
        .bytes(),
    );
    let path = dir.join("encrypted.pdf");
    std::fs::write(&path, out).unwrap();
    path
}
