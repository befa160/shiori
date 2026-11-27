use super::*;

#[test]
fn the_first_line_of_the_status_gives_the_publication_status() {
    assert_eq!(status("7 Volumes (Complete)").as_deref(), Some("completed"));
    assert_eq!(
        status("12 Volumes (Ongoing)\n\n3 Volumes (Complete) Part 2").as_deref(),
        Some("ongoing")
    );
    assert_eq!(status("4 Volumes (Hiatus)").as_deref(), Some("hiatus"));
    assert_eq!(
        status("2 Volumes (Cancelled)").as_deref(),
        Some("cancelled")
    );
    assert_eq!(
        status("1 Volume (Discontinued)").as_deref(),
        Some("cancelled")
    );
    assert_eq!(status("3 Volumes (Incomplete)"), None);
    assert_eq!(status("N/A"), None);
    assert_eq!(status(""), None);
}

#[test]
fn the_volume_count_is_the_number_before_volumes_on_the_first_line() {
    assert_eq!(volume_count("7 Volumes (Complete)"), Some(7));
    assert_eq!(volume_count("1 Volume (Complete)"), Some(1));
    assert_eq!(
        volume_count("12 Volumes (Ongoing)\n\n3 Volumes Part 2"),
        Some(12)
    );
    assert_eq!(volume_count("45 Chapters (Ongoing)"), None);
    assert_eq!(volume_count("N/A"), None);
    assert_eq!(volume_count("0 Volumes"), None);
}
