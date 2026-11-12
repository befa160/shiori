//! Archive CBR (RAR) en lecture seule, via `rars` (pur Rust, jamais unrar) : voir
//! `docs/cbr.md`. Une archive non solid se lit membre par membre ; une archive solid n'a
//! pas d'accès direct (lire une page décode toutes celles d'avant), d'où [`Archive::read_each`]
//! qui décode tout le tome en une seule passe.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

use rars::{
    ArchiveFamily, ArchiveMemberDetail, ArchiveReadOptions, ArchiveReader, ErrorKind,
    ExtractionDecision, ExtractionErrorAction,
};

use super::{Archive, ArchiveError, Duplicate, Entry, EntryKind, MAX_ENTRY_SIZE};

/// Plafond d'en-têtes lus à l'ouverture : une archive piégée échoue au lieu de remplir la
/// mémoire. Un tome réel en compte quelques centaines.
const MAX_HEADERS: u64 = 1_000_000;

pub struct RarArchive {
    rar: rars::Archive,
    /// Tous les membres, dans l'ordre de l'archive : `entries[i].index == i`.
    entries: Vec<Entry>,
    solid: bool,
    multivolume: bool,
}

impl RarArchive {
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        let options = ArchiveReadOptions::new().with_max_header_count(MAX_HEADERS);
        let rar = ArchiveReader::read_path_with_options(path, options).map_err(|e| error(&e))?;
        let mut multivolume = false;
        let entries = rar
            .members()
            .enumerate()
            .map(|(index, m)| {
                multivolume |= m.meta.is_split_before || m.meta.is_split_after;
                let mut name = m.decoded_name(None).map_or_else(
                    |_| m.meta.name_lossy(),
                    |n| String::from_utf8_lossy(&n).into(),
                );
                // RAR 1.5-4.x sépare les dossiers par `\` : même convention que le zip.
                if m.meta.family != ArchiveFamily::Rar50Plus {
                    name = name.replace('\\', "/");
                }
                let crc32 = match m.detail {
                    ArchiveMemberDetail::Rar15To40 { crc32, .. } => crc32,
                    ArchiveMemberDetail::Rar50Plus { crc32, .. } => crc32.unwrap_or(0),
                    _ => 0,
                };
                Entry {
                    index,
                    name,
                    kind: if m.meta.is_directory {
                        EntryKind::Dir
                    } else if m.meta.is_redirection {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    },
                    compressed_size: m.meta.packed_size,
                    size: m.meta.unpacked_size,
                    crc32,
                    compression: if m.meta.is_stored { "stored" } else { "rar" }.into(),
                    compression_supported: true,
                    encrypted: m.meta.is_encrypted,
                    name_raw: m.meta.name,
                }
            })
            .collect();
        Ok(Self {
            solid: rar.is_solid(),
            rar,
            entries,
            multivolume,
        })
    }

    /// Le décodage ne produit jamais plus que la somme des tailles déclarées.
    fn options(&self) -> ArchiveReadOptions<'static> {
        let total = self
            .entries
            .iter()
            .fold(0u64, |sum, e| sum.saturating_add(e.size));
        ArchiveReadOptions::new().with_max_total_output_bytes(total)
    }
}

impl Archive for RarArchive {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn duplicates(&self) -> Result<Vec<Duplicate>, ArchiveError> {
        let mut by_name: BTreeMap<&[u8], Vec<&Entry>> = BTreeMap::new();
        for e in &self.entries {
            by_name.entry(&e.name_raw).or_default().push(e);
        }
        Ok(by_name
            .into_values()
            .filter(|same| same.len() > 1)
            .map(|same| Duplicate {
                name: same[0].name.clone(),
                name_raw: same[0].name_raw.clone(),
                identical: same
                    .windows(2)
                    .all(|w| (w[0].crc32, w[0].size) == (w[1].crc32, w[1].size)),
            })
            .collect())
    }

    fn open_entry(&mut self, index: usize) -> Result<Box<dyn io::Read + '_>, ArchiveError> {
        Ok(Box::new(io::Cursor::new(self.read_entry(index)?)))
    }

    /// Lecture directe d'un membre ; sur une archive solid, décode aussi tous ceux d'avant.
    fn read_entry(&mut self, index: usize) -> Result<Vec<u8>, ArchiveError> {
        let entry = self
            .entries
            .get(index)
            .ok_or(ArchiveError::NoSuchEntry(index))?;
        if entry.size > MAX_ENTRY_SIZE {
            return Err(ArchiveError::EntryTooLarge(entry.size));
        }
        if entry.encrypted {
            return Err(ArchiveError::Encrypted);
        }
        let bytes = self
            .rar
            .read_member_at_with_options(index, self.options())
            .map_err(|e| error(&e))?
            .ok_or(ArchiveError::NoSuchEntry(index))?;
        // `rars` vérifie le CRC mais tolère un contenu plus long que déclaré.
        if bytes.len() as u64 != entry.size {
            return Err(ArchiveError::SizeMismatch);
        }
        Ok(bytes)
    }

    fn is_solid(&self) -> bool {
        self.solid
    }

    fn is_multivolume(&self) -> bool {
        self.multivolume
    }

    /// Une seule passe de décodage en flux pour tout le tome, un membre en mémoire à la
    /// fois. Le contenu d'un membre n'est remis qu'une fois son CRC vérifié, c'est-à-dire
    /// quand `rars` passe au membre suivant ou termine.
    fn read_each(
        &mut self,
        indices: &[usize],
        visit: &mut dyn FnMut(usize, Result<Vec<u8>, ArchiveError>),
    ) {
        let Some(&last) = indices.iter().max() else {
            return;
        };
        let wanted: BTreeMap<usize, u64> = indices
            .iter()
            .filter_map(|&i| Some((i, self.entries.get(i)?.size)))
            .collect();
        let state = RefCell::new(Stream {
            visit,
            seen: 0,
            current: None,
            visited: BTreeSet::new(),
        });
        let solid = self.solid;
        let result = self.rar.extract_with_control_and_errors(
            self.options(),
            |member| {
                let mut s = state.borrow_mut();
                s.flush(None);
                let index = s.seen;
                s.seen += 1;
                if index > last {
                    return Ok(ExtractionDecision::Stop);
                }
                if member.meta.is_directory || member.meta.is_redirection {
                    return Ok(ExtractionDecision::Skip);
                }
                let skip = || match solid {
                    // L'historique d'une archive solid se décode même pour les pages ignorées.
                    true => ExtractionDecision::Extract(Box::new(io::sink())),
                    false => ExtractionDecision::Skip,
                };
                let Some(&size) = wanted.get(&index) else {
                    return Ok(skip());
                };
                if size > MAX_ENTRY_SIZE {
                    s.done(index, Err(ArchiveError::EntryTooLarge(size)));
                    return Ok(skip());
                }
                if member.meta.is_encrypted {
                    s.done(index, Err(ArchiveError::Encrypted));
                    return Ok(skip());
                }
                let buffer = Rc::new(RefCell::new(Capped {
                    bytes: Vec::with_capacity(size as usize),
                    max: size,
                    overflow: false,
                }));
                s.current = Some((index, buffer.clone()));
                Ok(ExtractionDecision::Extract(Box::new(Shared(buffer))))
            },
            |_, err| {
                state.borrow_mut().flush(Some(error(err)));
                Ok(ExtractionErrorAction::Continue)
            },
        );
        let mut s = state.into_inner();
        match result {
            Ok(_) => {
                s.flush(None);
                s.rest(indices, ArchiveError::NoSuchEntry);
            }
            Err(e) => {
                s.flush(Some(error(&e)));
                // Archive solid : les pages suivantes dépendent de celle qui a échoué.
                s.rest(indices, |_| match error(&e) {
                    ArchiveError::Truncated(m) => ArchiveError::Truncated(m),
                    other => ArchiveError::Unreadable(format!(
                        "décodage interrompu plus tôt dans l'archive : {other}"
                    )),
                });
            }
        }
    }
}

/// État partagé par les deux rappels de `rars` pendant [`RarArchive::read_each`].
struct Stream<'a> {
    visit: &'a mut dyn FnMut(usize, Result<Vec<u8>, ArchiveError>),
    /// Membres déjà présentés par `rars`.
    seen: usize,
    /// Membre en cours d'écriture et son tampon.
    current: Option<(usize, Rc<RefCell<Capped>>)>,
    visited: BTreeSet<usize>,
}

impl Stream<'_> {
    fn done(&mut self, index: usize, result: Result<Vec<u8>, ArchiveError>) {
        self.visited.insert(index);
        (self.visit)(index, result);
    }

    /// Remet le membre en cours : son contenu, ou `failed`.
    fn flush(&mut self, failed: Option<ArchiveError>) {
        let Some((index, buffer)) = self.current.take() else {
            return;
        };
        let buffer = std::mem::take(&mut *buffer.borrow_mut());
        // Le refus du tampon remonte de `rars` en erreur d'E/S : c'est lui la vraie cause.
        let result = match failed {
            _ if buffer.overflow => Err(ArchiveError::SizeMismatch),
            Some(e) => Err(e),
            None if buffer.bytes.len() as u64 != buffer.max => Err(ArchiveError::SizeMismatch),
            None => Ok(buffer.bytes),
        };
        self.done(index, result);
    }

    /// Les membres demandés jamais atteints.
    fn rest(&mut self, indices: &[usize], error: impl Fn(usize) -> ArchiveError) {
        for &i in indices {
            if self.visited.insert(i) {
                (self.visit)(i, Err(error(i)));
            }
        }
    }
}

/// Tampon d'un membre, borné par sa taille déclarée.
#[derive(Default)]
struct Capped {
    bytes: Vec<u8>,
    max: u64,
    overflow: bool,
}

struct Shared(Rc<RefCell<Capped>>);

impl Write for Shared {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut b = self.0.borrow_mut();
        if b.bytes.len() as u64 + data.len() as u64 > b.max {
            b.overflow = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "membre plus long que sa taille déclarée",
            ));
        }
        b.bytes.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Erreur `rars` vers les catégories du scan, d'après sa catégorie stable.
fn error(e: &rars::Error) -> ArchiveError {
    let message = e.to_string();
    match (e.kind(), e.root_cause()) {
        (_, rars::Error::TooShort) => ArchiveError::Truncated(message),
        (_, rars::Error::TotalOutputLimitExceeded { .. }) => ArchiveError::SizeMismatch,
        (ErrorKind::ChecksumMismatch, _) => ArchiveError::CrcMismatch,
        (ErrorKind::PasswordRequired | ErrorKind::BadPassword, _) => ArchiveError::Encrypted,
        (ErrorKind::UnsupportedFormat | ErrorKind::UnsupportedFeature, _) => {
            ArchiveError::UnsupportedCompression(message)
        }
        (ErrorKind::Io, rars::Error::Io(io)) if io.kind == io::ErrorKind::UnexpectedEof => {
            ArchiveError::Truncated(message)
        }
        (ErrorKind::Io, _) => ArchiveError::Io(io::Error::other(message)),
        _ => ArchiveError::Unreadable(message),
    }
}

#[cfg(test)]
mod tests;
