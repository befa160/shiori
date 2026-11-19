# Libraries and series

## Libraries

A **library** is a folder on your disk, with a type that sets the default reading direction:

| Type | Default reading direction |
|---|---|
| Manga | right to left, page by page |
| Comics | left to right, page by page |
| Webtoon | continuous vertical scrolling |

Each **subfolder** of the library is a **series**, and each `.cbz`, `.cbr` or `.pdf` file in it is a **volume** (or a chapter). Libraries are created in **Administration → Add a library**; two libraries cannot overlap.

A new library is only visible to administrators, unless **Give access to all readers** is checked when creating it; otherwise, open it to the accounts you want in **Administration → Accounts**.

A volume is recognized by its **content**, not its name: renaming or moving a file, or renaming a series folder, loses neither progress nor metadata.

## Organizing your files

```
Manga/                          ← the library
├── One Piece/                  ← one series per folder
│   ├── One Piece T01.cbz
│   ├── One Piece T02.cbz
│   └── One Piece HS 01.cbz     ← special, sorted after the volumes
└── Blame!/
    ├── Blame! v01.cbr
    └── Blame! v02.cbr
```

Shiori reads the **number** from the file name. It recognizes, case-insensitively, with or without dots or spaces:

- **volumes**: `Tome 3`, `T03`, `Vol 3`, `Volume 3`, `v03`, `Intégrale`, `第3巻`;
- **chapters**: `Chapitre 12`, `Ch 12`, `Chapter 12`, `c012`, `#12`, `第12話`;
- **specials**: `HS`, `Hors-série`, `Special`, `Extra`, `Omake`, `One-shot`;
- **numbers**: `03` or `3`, decimals (`12.5`, `12,5`), ranges (`01-03`, `01 à 03`); otherwise, a lone number at the end of the name (`Blame 03.cbz` → volume 3).

Anything in square brackets or parentheses is ignored (`[Team]`, `(2019)`, `(Digital)`), and a lone year is never taken for a number. Within a series, the order is: volumes, then chapters outside volumes, then specials, then files without a recognized number, each by number then by name.

Files placed directly at the root of the library are grouped by series name; they join the series folder of the same name if there is one.

## Formats

- **CBZ**: zip, very large files included (zip64).
- **CBR**: RAR 4 and 5, "solid" included (the first page opens a bit more slowly). Multi-volume archives (`.part1.rar`) are not read.
- **PDF**: Shiori shows the embedded images, **without the margins** of the PDF page; on a tablet, the artwork fills the screen. A page made of several pieces (strips, tiles) is reassembled. A text-only page, or one whose image covers less than half of it, is skipped and reported; text on top of artwork (credits, preface) is not shown. A password-protected PDF is not read.
- **Images**: JPEG, PNG, WebP, AVIF. Pages are sorted in the natural order of their names (`2` before `10`), whatever their order in the archive.

## What Shiori does not do

- It **never** modifies, moves or renames your files.
- It does not follow **symbolic links**: mount the folder itself (Docker volume, hard link, network mount).
- If a library's folder is missing, unreadable or suddenly empty (network drive not mounted), the scan fails without removing anything: series and settings are kept until the files come back. Volumes in an unreadable subfolder are kept as they are, with a warning in the scan log.
