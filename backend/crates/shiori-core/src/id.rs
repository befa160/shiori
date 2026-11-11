//! Identités stables des tomes et des pages (principes 1 et 8).
//!
//! - Un tome est identifié par le hash BLAKE3 de son contenu complet : renommer ou
//!   déplacer le fichier ne change pas son identité.
//! - Une page est identifiée par `BLAKE3(volume_id ‖ 0x00 ‖ nom brut de l'entrée)`,
//!   tronqué à 128 bits. On utilise les octets bruts du nom stocké dans l'archive :
//!   aucune heuristique de décodage (CP437, UTF-8…) ne peut changer un ID.
//!
//! Ces formules sont figées : les modifier invaliderait toutes les URLs et toutes
//! les progressions enregistrées. Les tests `*_is_frozen` le garantissent.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::str::FromStr;

/// Identité d'un tome : BLAKE3 du contenu complet du fichier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VolumeId([u8; 32]);

/// Identité d'une page : 128 premiers bits de `BLAKE3(volume_id ‖ 0x00 ‖ nom brut)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageId([u8; 16]);

impl VolumeId {
    /// Hash d'un flux complet (lu jusqu'à la fin).
    pub fn from_reader(reader: impl Read) -> io::Result<Self> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_reader(reader)?;
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    /// Hash d'un fichier, ouvert en lecture seule.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        Self::from_reader(File::open(path)?)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PageId {
    pub fn derive(volume: &VolumeId, entry_name_raw: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(volume.as_bytes());
        hasher.update(&[0]);
        hasher.update(entry_name_raw);
        let mut id = [0; 16];
        id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
        Self(id)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Erreur de lecture d'un identifiant hexadécimal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("identifiant invalide : {expected} caractères hexadécimaux minuscules attendus")]
pub struct ParseIdError {
    expected: usize,
}

fn write_hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
}

/// Accepte uniquement l'hexadécimal minuscule : une seule écriture possible par ID,
/// donc une seule URL par image.
fn parse_hex<const N: usize>(s: &str) -> Result<[u8; N], ParseIdError> {
    let err = ParseIdError { expected: N * 2 };
    let digits = s.as_bytes();
    if digits.len() != N * 2 {
        return Err(err);
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(err.clone()),
    };
    let mut out = [0; N];
    let (pairs, _) = digits.as_chunks::<2>();
    for (byte, &[hi, lo]) in out.iter_mut().zip(pairs) {
        *byte = (nibble(hi)? << 4) | nibble(lo)?;
    }
    Ok(out)
}

macro_rules! hex_id {
    ($ty:ident, $len:literal) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex(f, &self.0)
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({self})", stringify!($ty))
            }
        }

        impl FromStr for $ty {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                parse_hex::<$len>(s).map(Self)
            }
        }
    };
}

hex_id!(VolumeId, 32);
hex_id!(PageId, 16);

#[cfg(test)]
mod tests;
