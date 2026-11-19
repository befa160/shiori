# Diagnostic codes

Every problem found by a scan has a **stable code** and a **severity**. A volume with at least one **error** is marked "Error"; with a **warning**, "Warning"; an **info** changes nothing in the display. A volume's problems are in its menu → **Scan issues**, and all affected files in **Administration → Library health**.

An erroneous page is never replaced by another one: the reader shows the error in its place, and the rest of the volume stays readable.

## Errors

| Code | What it means | What to do |
|---|---|---|
| `archive_unreadable` | The file is not a valid CBZ, CBR or PDF, or cannot be read (permissions, disk). | Download or recreate the archive again; check permissions. |
| `archive_truncated` | The file is cut (incomplete download or copy). | Download or copy the file again. |
| `multivolume_archive` | Multi-volume archive (`.part1.rar`…). | Merge it into a single archive. |
| `encrypted_entry` | A page, or the whole PDF, is password-protected. | Recreate the archive or PDF without a password. |
| `unsupported_compression` | Unknown compression method. | Recreate the archive (standard zip). |
| `crc_mismatch` | A page's content does not match the archive: damaged page. | Download the file again. |
| `size_mismatch` | A page's size does not match the archive: damaged page. | Download the file again. |
| `entry_too_large` | A page is too large to be read safely. | Shrink the image and recreate the archive. |
| `image_undecodable` | An image cannot be decoded (or, in a PDF, announces too many pixels). | Replace the damaged image. |
| `duplicate_entry_conflict` | Two pages have the same name with different contents. | Rename one of them in the archive. |
| `no_pages` | No readable page in the file. | Check the archive's content. |
| `file_changed_during_scan` | The file changed while being read (copy in progress). | Scan again once the copy is done (watching does it on its own). |

## Warnings

| Code | What it means |
|---|---|
| `duplicate_entry_identical` | A page is duplicated with identical content; only one is kept. |
| `extension_mismatch` | The extension does not match the image's real format (a `.jpg` that is a PNG); the page is read anyway. |
| `unsupported_image` | Image in an unsupported format (GIF, BMP, TIFF, JPEG XL, HEIC): page ignored. |
| `non_image_entry` | A file that is not an image: ignored. |
| `subdirectories` | Pages are stored in subfolders of the archive; they are read anyway. |
| `archive_order_differs` | The order of pages in the archive differs from the reading order; Shiori follows the order of names. |
| `metadata_invalid` | An unreadable `ComicInfo.xml` or `index.json`: ignored, the volume stays readable. |
| `pdf_page_without_image` | A PDF page without an image (text only), or whose image covers less than half of it: skipped. |

## Information

| Code | What it means |
|---|---|
| `ignored_entry` | A known side file (`ComicInfo.xml`, `Thumbs.db`, `.DS_Store`, `__MACOSX` folder) was ignored. |
| `solid_archive` | Solid RAR archive: the first page takes a little longer to open. |
| `pdf_text_ignored` | The visible text of a PDF page is not shown: only the image is. |
