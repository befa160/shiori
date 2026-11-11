use super::*;

#[test]
fn user_dirs_come_from_the_variable_then_from_the_os() {
    let pick: fn(&ProjectDirs) -> &Path = |d| d.cache_dir();
    let set = user_dir("X", Some("/mon/cache".into()), pick).unwrap();
    assert_eq!(set, PathBuf::from("/mon/cache"));
    // Vide ou absente : dossier de l'OS, propre à Shiori.
    for value in [Some(OsString::new()), None] {
        let os = user_dir("X", value, pick).unwrap();
        assert!(
            os.to_string_lossy().to_lowercase().contains("shiori"),
            "{}",
            os.display()
        );
    }
    assert!(cache_dir().is_ok());
}
