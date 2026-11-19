# Archives and scan

Each format implements the `Archive` trait of `shiori-core`: list of entries in archive order, reading an entry by its index, CRC and size check. The format is chosen from the first bytes of the file, never from the extension: a zip renamed `.cbr` stays a zip.

## CBZ

`zip` crate: reading an entry through the central directory, without full extraction; zip64 and large files supported. Each page is checked against the directory's CRC and size.

## CBR

`rars` crate (pure Rust, never `unrar`): RAR 4 and 5.

- **Non-solid**: direct read of a member by its index, same checks as CBZ.
- **Solid** (no random access): on the first request for a volume, one pass extracts all its entries into `<cache>/extract/<volume_id>/<index>` (temporary folder renamed at the end: never a partial folder visible). Each file is checked against the scan's size before being served. One extraction at a time. Extracted volumes share the `--cache-max-mib` cap with renders; the least recently read goes first, never the last volume read.
- **Limits**: 1,000,000 headers at most, output bounded to the sum of declared sizes, each page in a buffer bounded to its declared size; an entry that is too large is never loaded in memory (`entry_too_large`). Multi-volume archives are refused (`multivolume_archive`), so is a cut RAR (`archive_truncated`).
- **Duplicate names**: RAR keeps them all; only the last occurrence becomes a page.

## PDF

`hayro-interpret` crate (pure Rust), read like an archive, without conversion: Shiori serves the **embedded images**, not the PDF page with its margins.

- A `Device` that draws nothing interprets each page (matrices, forms, `/Rotate`, colors, transparency) and records its images and text.
- A page whose images cover at least half of its area gives an entry `NNNN.jpg` or `NNNN.png` (PDF page number); the others are set aside with `pdf_page_without_image`.
- An upright JPEG, gray or RGB, without a mask, is served **as is**: its position in the file is recorded when opening, then read directly (CRC checked again).
- Any other page is decoded then encoded as PNG: image straightened, transparency kept, a page made of several images (strips, tiles, layers) **recomposed** at the density of the finest image and cropped to the union of the images.
- Safeguards: `%%EOF` required (otherwise `archive_truncated`), 128 Mpx at most per page, pieces smaller than a thousandth of the page discarded. An encrypted PDF is an error (`encrypted_entry`).

## Integrity scan

`shiori-core::integrity::scan_file` produces one report per file:

- CRC and size of each entry, image headers and dimensions (`imagesize`, without decoding), duplicates, stray files, subfolders, order, truncated or encrypted archives; `--deep` decodes every image.
- Each problem has a stable code and a severity ([diagnostic codes](/en/guide/diagnostics)). An unreadable page is marked `broken`; the rest of the volume stays readable.
- A panic in a third-party library (malformed archive) becomes a problem of the file, never the end of the scan.
- A file modified while being read has no reliable volume (`file_changed_during_scan`): nothing is attached, it will be read again.
- **File metadata**: `ComicInfo.xml` and `index.json` read during the scan (solid RAR included), and a PDF's `Info` dictionary (authors, subject → summary, keywords → tags; neither title nor date); an unreadable file is a warning, never a volume error.

A rescan skips the hash: a file whose size and date have not changed is counted "unchanged". `SCANNER_VERSION` (scan rules) and `PARSER_VERSION` (name parser), frozen by tests, restart the analysis when they change.

## Name parser

`shiori-core::name::parse(relative path)` is a pure function: series = first folder under the root, then type (volume, chapter, special, unknown), number and range end from French, English and Japanese markers. Rules and examples: [Organizing your files](/en/guide/libraries#organizing-your-files).
