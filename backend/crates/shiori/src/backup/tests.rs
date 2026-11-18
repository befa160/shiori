use super::*;

fn backups(dir: &Path, keep: usize) -> Backups {
    Backups {
        dir: dir.join("backups"),
        every: Some(Duration::from_secs(3600)),
        keep,
    }
}

#[test]
fn names_outside_the_folder_or_hidden_are_refused() {
    for name in [
        "shiori-20261118-101010.db",
        "shiori-avant-0018-x.db",
        "a_b.c.db",
    ] {
        assert!(valid_name(name), "{name}");
    }
    for name in [
        "../shiori.db",
        "a/b.db",
        "a\\b.db",
        ".part.db",
        "shiori.txt",
        "été.db",
        "",
    ] {
        assert!(!valid_name(name), "{name}");
    }
    assert!(is_routine("shiori-20261118-101010.db"));
    for name in [
        "shiori-avant-0018-1.db",
        "shiori-importee-1.db",
        "autre-2026.db",
        "shiori.db",
    ] {
        assert!(!is_routine(name), "{name}");
    }
}

#[tokio::test]
async fn only_the_latest_routine_backups_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("shiori.db")).await.unwrap();
    let b = backups(dir.path(), 2);
    assert_eq!(b.list(), []);
    assert_eq!(b.due_in(Duration::from_secs(3600)), Duration::ZERO);
    std::fs::create_dir_all(&b.dir).unwrap();
    for name in [
        "shiori-20200101-000000.db",
        "shiori-20200102-000000.db",
        "shiori-avant-0017-20200101-000000.db",
        "notes.txt",
    ] {
        std::fs::write(b.dir.join(name), b"x").unwrap();
    }
    std::fs::create_dir(b.dir.join("dossier.db")).unwrap();
    let made = b.create(&db).await.unwrap();
    assert!(made.size > 0);
    let mut names: Vec<String> = b.list().into_iter().map(|f| f.name).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "shiori-20200102-000000.db".to_owned(),
            made.name.clone(),
            "shiori-avant-0017-20200101-000000.db".to_owned(),
        ]
    );
    // Plus récente d'abord ; la prochaine périodique dans environ une heure.
    assert_eq!(b.list()[0].name, made.name);
    assert!(b.due_in(Duration::from_secs(3600)) > Duration::from_secs(3500));
    assert_eq!(b.path(&made.name), Some(b.dir.join(&made.name)));
    assert_eq!(b.path("absente.db"), None);
    assert_eq!(b.path("../shiori.db"), None);
    assert_eq!(b.path("dossier.db"), None);
}

#[tokio::test]
async fn the_periodic_backup_runs_at_once_then_on_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("shiori.db")).await.unwrap());
    let b = backups(dir.path(), 7);
    Backups {
        every: None,
        ..b.clone()
    }
    .spawn(db.clone());
    b.clone().spawn(db);
    // Aucune sauvegarde encore : la première tout de suite, la suivante dans une heure.
    for _ in 0..500 {
        if !b.list().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(b.list().len(), 1);
}
