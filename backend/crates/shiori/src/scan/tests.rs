use super::*;
use shiori_core::{Issue, IssueCode};

fn outcome(status: Status, skipped: bool, codes: &[IssueCode]) -> Outcome {
    Outcome::new(scanned(status, skipped, codes), false)
}

fn scanned(status: Status, skipped: bool, codes: &[IssueCode]) -> Scanned {
    Scanned {
        report: FileReport {
            path: "x.cbz".into(),
            volume_id: None,
            format: "cbz",
            size: 0,
            status,
            pages: Vec::new(),
            issues: codes
                .iter()
                .map(|&code| Issue {
                    code,
                    severity: code.severity(),
                    entry: None,
                    message: String::new(),
                })
                .collect(),
        },
        skipped,
    }
}

#[test]
fn exit_code_follows_the_threshold() {
    let ok = outcome(Status::Ok, false, &[]);
    let warn = outcome(Status::Warning, false, &[]);
    let err = outcome(Status::Error, false, &[]);
    let code = |r: &[Outcome], f| exit_code(r, f);
    assert_eq!(code(&[], FailOn::Error), ExitCode::SUCCESS);
    assert_eq!(
        code(std::slice::from_ref(&warn), FailOn::Error),
        ExitCode::SUCCESS
    );
    assert_eq!(
        code(std::slice::from_ref(&warn), FailOn::Warning),
        ExitCode::from(1)
    );
    assert_eq!(
        code(std::slice::from_ref(&err), FailOn::Error),
        ExitCode::from(1)
    );
    assert_eq!(
        code(std::slice::from_ref(&ok), FailOn::Warning),
        ExitCode::SUCCESS
    );
}

#[test]
fn summary_counts_statuses_and_skips() {
    let results = [
        outcome(Status::Ok, true, &[]),
        outcome(Status::Warning, false, &[]),
        outcome(Status::Error, true, &[]),
    ];
    assert_eq!(
        Summary::of(&results),
        Summary {
            files: 3,
            ok: 1,
            warning: 1,
            error: 1,
            skipped: 2
        }
    );
}

#[test]
fn diagnostics_group_codes_and_hide_infos() {
    let r = scanned(
        Status::Warning,
        false,
        &[
            IssueCode::NonImageEntry,
            IssueCode::IgnoredEntry,
            IssueCode::NonImageEntry,
            IssueCode::Subdirectories,
        ],
    );
    assert_eq!(diagnostics(&r.report), "non_image_entry ×2, subdirectories");
    assert_eq!(
        diagnostics(&scanned(Status::Ok, false, &[IssueCode::IgnoredEntry]).report),
        "—"
    );
}

#[test]
fn only_comic_files_are_collected_sorted_and_deduplicated() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("b/sous")).unwrap();
    for f in [
        "b/sous/2.cbz",
        "a.CBZ",
        "b/1.cbz",
        "notes.txt",
        "c.CbR",
        "d.rar",
    ] {
        fs::write(root.join(f), b"x").unwrap();
    }
    let explicit = root.join("notes.txt");
    let found = collect(&[root.to_path_buf(), root.join("b"), explicit.clone()]).unwrap();
    let shown: Vec<String> = found.iter().map(|t| t.shown.replace('\\', "/")).collect();
    let expected = [
        "a.CBZ".to_string(),
        "b/1.cbz".into(),
        "b/sous/2.cbz".into(),
        "c.CbR".into(),
        explicit.display().to_string().replace('\\', "/"),
    ];
    assert_eq!(shown, expected);
}

#[test]
fn collect_rejects_missing_paths_and_empty_results() {
    let dir = tempfile::tempdir().unwrap();
    assert!(collect(&[dir.path().join("absent")]).is_err());
    assert!(collect(&[dir.path().to_path_buf()]).is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_inside_folders_are_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("dehors.cbz"), b"x").unwrap();
    fs::write(dir.path().join("dedans.cbz"), b"x").unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("lien")).unwrap();
    let found = collect(&[dir.path().to_path_buf()]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].shown, "dedans.cbz");
}

#[test]
fn labels_and_summary_line_are_readable() {
    assert_eq!(status_label(Status::Warning), "avertissement");
    let line = summary_line(
        &Summary {
            files: 2,
            ok: 1,
            warning: 0,
            error: 1,
            skipped: 1,
        },
        0.5,
    );
    assert_eq!(
        line,
        "2 fichier(s) : 1 ok, 0 avertissement(s), 1 erreur(s) ; 1 inchangé(s) non rescanné(s) ; 0.5 s"
    );
}

#[test]
fn pages_are_dropped_unless_kept_but_counted() {
    let mut s = scanned(Status::Ok, false, &[]);
    s.report.pages = vec![shiori_core::Page {
        id: shiori_core::PageId::derive(
            &shiori_core::VolumeId::from_reader(&b""[..]).unwrap(),
            b"p",
        ),
        ordinal: 0,
        entry_name: "p".into(),
        entry_name_raw: b"p".to_vec(),
        archive_index: 0,
        media_type: None,
        width: None,
        height: None,
        compressed_size: 0,
        size: 0,
        crc32: 0,
        status: shiori_core::PageStatus::Ok,
    }];
    let kept = Outcome::new(
        Scanned {
            report: s.report.clone(),
            skipped: false,
        },
        true,
    );
    let dropped = Outcome::new(s, false);
    assert_eq!((kept.page_count, kept.scanned.report.pages.len()), (1, 1));
    assert_eq!(
        (dropped.page_count, dropped.scanned.report.pages.len()),
        (1, 0)
    );
}
