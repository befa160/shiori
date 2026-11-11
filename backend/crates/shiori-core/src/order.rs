//! Ordre de lecture des pages : tri naturel du chemin complet dans l'archive.
//!
//! Le chemin est comparé composante par composante (`/` et `\` sont tous deux des
//! séparateurs, quel que soit l'OS qui a créé l'archive). Chaque composante est
//! comparée en tri naturel insensible à la casse (`2.jpg` < `10.jpg`), puis on départage
//! par la casse et enfin par les octets bruts du nom : l'ordre est total, donc
//! deux scans du même fichier produisent toujours le même ordre.

use std::cmp::Ordering;

fn components(name: &str) -> impl Iterator<Item = &str> {
    name.split(['/', '\\']).filter(|c| !c.is_empty())
}

fn natural(a: &str, b: &str) -> Ordering {
    alphanumeric_sort::compare_str(a.to_lowercase(), b.to_lowercase())
        .then_with(|| alphanumeric_sort::compare_str(a, b))
}

/// Compare deux noms d'entrée selon l'ordre de lecture.
pub fn compare_entry_names(a: &str, a_raw: &[u8], b: &str, b_raw: &[u8]) -> Ordering {
    let mut ca = components(a);
    let mut cb = components(b);
    loop {
        match (ca.next(), cb.next()) {
            (Some(x), Some(y)) => match natural(x, y) {
                Ordering::Equal => continue,
                other => return other,
            },
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => return a_raw.cmp(b_raw),
        }
    }
}

#[cfg(test)]
mod tests;
