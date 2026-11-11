//! `shiori scan` de bout en bout : le vrai binaire, de vraies fixtures.

use std::path::Path;
use std::process::{Command, Output};

use shiori_testkit::fixtures;

fn shiori(args: &[&str], data_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shiori"))
        .args(args)
        .env("SHIORI_DATA_DIR", data_dir)
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Une petite bibliothèque : un tome sain, un avec avertissement, deux cassés.
fn library(root: &Path) {
    let series = root.join("Série A");
    std::fs::create_dir_all(&series).unwrap();
    fixtures::manga(&series);
    fixtures::unpadded(&series);
    fixtures::bad_crc(root);
    fixtures::not_a_zip(root);
    std::fs::write(root.join("lisez-moi.txt"), "pas une archive").unwrap();
}

#[test]
fn scans_a_library_then_skips_unchanged_files() {
    let lib = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    library(lib.path());
    let root = lib.path().to_str().unwrap();

    let first = shiori(&["scan", root], data.path());
    assert_eq!(first.status.code(), Some(1), "{}", stderr(&first));
    let out = stdout(&first);
    for expected in [
        "manga.cbz",
        "unpadded.cbz",
        "bad-crc.cbz",
        "not-a-zip.cbz",
        "crc_mismatch",
        "archive_order_differs",
    ] {
        assert!(out.contains(expected), "{expected} absent de :\n{out}");
    }
    assert!(!out.contains("lisez-moi"), "{out}");
    assert!(
        out.contains("4 fichier(s) : 1 ok, 1 avertissement(s), 2 erreur(s) ; 0 inchangé(s)"),
        "{out}"
    );
    assert!(
        data.path().join("shiori.db").exists(),
        "base créée dans SHIORI_DATA_DIR"
    );

    let second = shiori(&["scan", root, "--json", "-"], data.path());
    assert_eq!(second.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(json["report_version"], 1);
    assert_eq!(json["summary"]["files"], 4);
    assert_eq!(json["summary"]["skipped"], 4);
    assert!(
        json["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["skipped"] == true)
    );
    // Avec le JSON sur la sortie standard, le tableau passe sur la sortie d'erreur.
    assert!(
        stderr(&second).contains("4 inchangé(s)"),
        "{}",
        stderr(&second)
    );
}

#[test]
fn fail_on_controls_the_exit_code() {
    let lib = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let ok = fixtures::manga(lib.path());
    let warn = fixtures::unpadded(lib.path());
    let (ok, warn) = (ok.to_str().unwrap(), warn.to_str().unwrap());

    assert_eq!(
        shiori(&["scan", ok, "--no-db"], data.path()).status.code(),
        Some(0)
    );
    assert_eq!(
        shiori(&["scan", warn, "--no-db"], data.path())
            .status
            .code(),
        Some(0)
    );
    let strict = shiori(
        &["scan", warn, "--no-db", "--fail-on", "warning"],
        data.path(),
    );
    assert_eq!(strict.status.code(), Some(1));
}

#[test]
fn no_db_writes_nothing_and_rescans() {
    let lib = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let path = fixtures::manga(lib.path());
    let path = path.to_str().unwrap();
    for _ in 0..2 {
        let o = shiori(&["scan", path, "--no-db", "--json", "-", "-q"], data.path());
        let json: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(json["summary"]["skipped"], 0);
        assert_eq!(json["options"]["database"], false);
        assert!(stderr(&o).is_empty(), "-q : rien d'autre que le JSON");
    }
    assert!(!data.path().join("shiori.db").exists());
}

#[test]
fn explicit_db_json_file_rehash_and_deep() {
    let lib = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let path = fixtures::corrupt_body(lib.path());
    let db = data.path().join("sous/dossier/base.db");
    let report = data.path().join("rapport.json");
    let (path, db_s, report_s) = (
        path.to_str().unwrap(),
        db.to_str().unwrap(),
        report.to_str().unwrap(),
    );

    let normal = shiori(&["scan", path, "--db", db_s, "-q"], data.path());
    assert_eq!(normal.status.code(), Some(0));
    assert!(stdout(&normal).is_empty());
    assert!(db.exists(), "dossiers parents créés");

    let deep = shiori(
        &[
            "scan", path, "--db", db_s, "--deep", "--rehash", "--json", report_s, "-v",
        ],
        data.path(),
    );
    assert_eq!(
        deep.status.code(),
        Some(1),
        "le scan approfondi trouve le JPEG tronqué"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(json["files"][0]["skipped"], false);
    assert_eq!(json["files"][0]["issues"][0]["code"], "image_undecodable");
    assert!(
        stderr(&deep).contains("scanné"),
        "journal -v : {}",
        stderr(&deep)
    );
}

#[test]
fn parallel_and_sequential_scans_agree() {
    let lib = tempfile::tempdir().unwrap();
    library(lib.path());
    let root = lib.path().to_str().unwrap();
    let run = |jobs: &str| {
        let data = tempfile::tempdir().unwrap();
        let o = shiori(
            &["scan", root, "--jobs", jobs, "--json", "-", "-q"],
            data.path(),
        );
        let mut json: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        json["files"].take()
    };
    assert_eq!(run("1"), run("8"));
}

#[test]
fn usage_errors_exit_with_2() {
    let data = tempfile::tempdir().unwrap();
    let missing = shiori(&["scan", "/chemin/qui/n/existe/pas"], data.path());
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        stderr(&missing).contains("introuvable"),
        "{}",
        stderr(&missing)
    );

    let empty = shiori(&["scan", data.path().to_str().unwrap()], data.path());
    assert_eq!(empty.status.code(), Some(2));
    assert!(stderr(&empty).contains("aucun fichier .cbz"));

    let conflict = shiori(&["scan", ".", "--db", "x.db", "--no-db"], data.path());
    assert_eq!(conflict.status.code(), Some(2));

    // Un dossier à la place du fichier de base : ouverture impossible.
    let lib = tempfile::tempdir().unwrap();
    let tome = fixtures::manga(lib.path());
    let bad_db = shiori(
        &[
            "scan",
            tome.to_str().unwrap(),
            "--db",
            data.path().to_str().unwrap(),
        ],
        data.path(),
    );
    assert_eq!(bad_db.status.code(), Some(2), "{}", stderr(&bad_db));
    assert!(
        stderr(&bad_db).contains("ouverture de la base"),
        "{}",
        stderr(&bad_db)
    );
}

/// Base indiquée par un simple nom de fichier : créée dans le dossier courant.
#[test]
fn db_given_as_a_bare_file_name() {
    let lib = tempfile::tempdir().unwrap();
    let tome = fixtures::manga(lib.path());
    let out = Command::new(env!("CARGO_BIN_EXE_shiori"))
        .args(["scan", tome.to_str().unwrap(), "--db", "nu.db", "-q"])
        .current_dir(lib.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(lib.path().join("nu.db").exists());
}

/// Sans `SHIORI_DATA_DIR` (ou vide), la base va dans le dossier de données de l'OS
/// (`$XDG_DATA_HOME/shiori` sous Linux ; ailleurs, le vrai dossier de l'utilisateur serait
/// touché, d'où la restriction).
#[cfg(target_os = "linux")]
#[test]
fn default_database_goes_to_the_os_data_dir() {
    let lib = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let tome = fixtures::manga(lib.path());
    let out = Command::new(env!("CARGO_BIN_EXE_shiori"))
        .args(["scan", tome.to_str().unwrap(), "-q"])
        .env("SHIORI_DATA_DIR", "")
        .env("XDG_DATA_HOME", xdg.path())
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(xdg.path().join("shiori/shiori.db").exists());
}
