use super::*;

fn sorted(names: &[&str]) -> Vec<String> {
    let mut v: Vec<&str> = names.to_vec();
    v.sort_by(|a, b| compare_entry_names(a, a.as_bytes(), b, b.as_bytes()));
    v.into_iter().map(String::from).collect()
}

#[test]
fn numbers_are_compared_by_value() {
    assert_eq!(
        sorted(&["10.jpg", "2.jpg", "1.jpg", "100.jpg", "02b.jpg"]),
        ["1.jpg", "2.jpg", "02b.jpg", "10.jpg", "100.jpg"]
    );
}

#[test]
fn padded_and_unpadded_mix() {
    assert_eq!(
        sorted(&["page 10.png", "page 9.png", "page 09.png", "page 1.png"]),
        // À valeur égale, la forme la plus courte passe d'abord : ordre stable.
        ["page 1.png", "page 9.png", "page 09.png", "page 10.png"]
    );
}

#[test]
fn case_insensitive_then_deterministic() {
    assert_eq!(
        sorted(&["b.jpg", "A.jpg", "a.jpg", "B.jpg"]),
        sorted(&["B.jpg", "a.jpg", "b.jpg", "A.jpg"])
    );
    let s = sorted(&["b.jpg", "A.jpg", "c.jpg"]);
    assert_eq!(s, ["A.jpg", "b.jpg", "c.jpg"]);
}

#[test]
fn subfolders_are_ordered_component_wise() {
    assert_eq!(
        sorted(&["ch10/01.jpg", "ch2/10.jpg", "ch2/9.jpg", "ch1/01.jpg"]),
        ["ch1/01.jpg", "ch2/9.jpg", "ch2/10.jpg", "ch10/01.jpg"]
    );
}

#[test]
fn backslash_is_a_separator() {
    assert_eq!(
        compare_entry_names("ch2\\1.jpg", b"ch2\\1.jpg", "ch2/1.jpg", b"ch2/1.jpg"),
        b"ch2\\1.jpg".as_slice().cmp(b"ch2/1.jpg")
    );
    assert_eq!(
        sorted(&["ch10\\1.jpg", "ch2\\1.jpg"]),
        ["ch2\\1.jpg", "ch10\\1.jpg"]
    );
}

#[test]
fn raw_bytes_break_remaining_ties() {
    // Même nom décodé, octets bruts différents (ex. encodages différents) : ordre stable.
    assert_eq!(
        compare_entry_names("é.jpg", &[0x82], "é.jpg", &[0xc3, 0xa9]),
        Ordering::Less
    );
}
