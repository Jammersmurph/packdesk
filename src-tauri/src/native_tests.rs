use super::*;
use sha2::{Digest, Sha256};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("mods")).unwrap();
    std::fs::write(
        dir.path().join("pack.toml"),
        r#"# A user's comment.
name = "Example Adventures"
author = "Example Author"
version = "1.0.0"
pack-format = "packwiz:1.1.0"
[index]
file = "index.toml"
hash-format = "sha256"
hash = ""
[versions]
minecraft = "1.21.1"
fabric = "0.16.10"
[options]
packdesk-channel = "beta"
acceptable-game-versions = ["1.21"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("index.toml"),
        r#"hash-format = "sha256"
[[files]]
file = "mods/example.pw.toml"
hash = ""
metafile = true
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("mods/example.pw.toml"),
        r#"# Keep this comment too.
name = "Example Landscapes"
filename = "example-landscapes-1.0.jar"
side = "both"
[download]
url = "https://example.com/example.jar"
hash-format = "sha512"
hash = "abc"
[update.modrinth]
mod-id = "example"
version = "version-one"
"#,
    )
    .unwrap();
    dir
}

#[test]
fn reads_native_files_without_conversion_or_writes() {
    let dir = fixture();
    let path = dir.path().join("pack.toml");
    let before = std::fs::read(&path).unwrap();
    let manifest = crate::manifest::read(dir.path()).unwrap();
    assert_eq!(manifest.name, "Example Adventures");
    assert_eq!(manifest.loader, "fabric");
    assert_eq!(manifest.channel.as_str(), "beta");
    let lock = crate::lockfile::read(dir.path()).unwrap();
    assert_eq!(lock.mods.len(), 1);
    assert_eq!(lock.mods[0].key, "mods/example.pw.toml");
    assert_eq!(lock.mods[0].version_id(), "version-one");
    assert_eq!(lock.mods[0].filename(), "example-landscapes-1.0.jar");
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert!(!dir.path().join("modpack.json").exists());
    assert!(!dir.path().join("content").exists());
    assert!(crate::convert::info(dir.path()).unwrap().is_none());
}

#[test]
fn edits_preserve_unknown_fields_comments_and_versions() {
    let dir = fixture();
    let mut manifest = read_manifest(dir.path()).unwrap();
    manifest.name = "Updated pack".into();
    write_manifest(dir.path(), &manifest).unwrap();
    let source = std::fs::read_to_string(dir.path().join(PACK_FILE)).unwrap();
    assert!(source.contains("# A user's comment."));
    assert!(source.contains("Example Author"));
    assert!(source.contains("acceptable-game-versions"));
    assert_eq!(read_manifest(dir.path()).unwrap().name, "Updated pack");
}

#[test]
fn honors_index_relative_paths() {
    let dir = fixture();
    std::fs::create_dir_all(dir.path().join("meta")).unwrap();
    std::fs::rename(dir.path().join("mods"), dir.path().join("meta/mods")).unwrap();
    std::fs::rename(
        dir.path().join("index.toml"),
        dir.path().join("meta/index.toml"),
    )
    .unwrap();
    let path = dir.path().join(PACK_FILE);
    let data = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\"index.toml\"", "\"meta/index.toml\"");
    std::fs::write(path, data).unwrap();
    assert_eq!(
        read_items(dir.path()).unwrap()[0].file,
        "meta/mods/example.pw.toml"
    );
}

#[test]
fn rejects_paths_outside_pack() {
    let dir = fixture();
    assert!(safe_path(dir.path(), "../other.toml").is_err());
    assert!(safe_path(dir.path(), "/etc/passwd").is_err());
    assert!(safe_path(dir.path(), "mods/example.pw.toml").is_ok());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escapes() {
    let dir = fixture();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
    assert!(safe_path(dir.path(), "escape/new-file.pw.toml").is_err());
}

#[test]
fn malformed_metadata_fails_instead_of_repairing_or_disappearing() {
    let dir = fixture();
    let path = dir.path().join("mods/example.pw.toml");
    std::fs::write(&path, "name = [bad").unwrap();
    assert!(crate::content::read_all_checked(dir.path()).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "name = [bad");
}

#[test]
fn refuses_initialization_in_nonempty_folder() {
    let dir = fixture();
    assert!(create(dir.path(), "New", "1.21.1", "vanilla", None).is_err());
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn selected_git_commit_uses_staged_content_not_unselected_working_edits() {
    let dir = fixture();
    let root = dir.path();
    refresh(root).unwrap();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.name", "Example Author"]);
    git(root, &["config", "user.email", "example@example.invalid"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "Initial pack"]);

    // Select the mod, but leave an unrelated config modification uncommitted.
    let path = root.join("mods/example.pw.toml");
    let source = std::fs::read_to_string(&path)
        .unwrap()
        .replace("version-one", "version-two");
    std::fs::write(&path, source).unwrap();
    std::fs::write(root.join("unselected.cfg"), "not in this commit\n").unwrap();
    refresh(root).unwrap();
    crate::git::commit(
        root,
        "Update selected mod",
        &["mods/example.pw.toml".into()],
        false,
    )
    .unwrap();

    let index = git(root, &["show", "HEAD:index.toml"]);
    assert!(!index.contains("unselected.cfg"));
    let staged_mod = git(root, &["show", "HEAD:mods/example.pw.toml"]);
    assert!(staged_mod.contains("version-two"));
    let digest = hex::encode(Sha256::digest(staged_mod.as_bytes()));
    assert!(index.contains(&digest));
    let pack = git(root, &["show", "HEAD:pack.toml"]);
    assert!(pack.contains(&hex::encode(Sha256::digest(index.as_bytes()))));
    assert!(root.join("unselected.cfg").exists());
    assert!(git(root, &["status", "--porcelain"]).contains("unselected.cfg"));
}

#[test]
fn real_packwiz_remove_refresh_and_undo_roundtrip() {
    let dir = fixture();
    let root = dir.path();
    refresh(root).unwrap();
    let snapshot = crate::undo::Snapshot::pack(root, "Remove mod");
    remove(root, "mods/example.pw.toml").unwrap();
    assert!(read_items(root).unwrap().is_empty());
    crate::undo::record(root, snapshot);
    crate::undo::undo(root).unwrap();
    assert_eq!(read_items(root).unwrap().len(), 1);
}

#[test]
fn installs_a_direct_url_through_real_packwiz_without_external_network() {
    use std::io::Read;
    use std::net::TcpListener;
    let dir = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let mut request = [0; 4096];
        let read = stream.read(&mut request).unwrap();
        assert!(read > 0);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nexample")
            .unwrap();
    });
    let item = ContentItem {
        name: "Local test download".into(),
        project_type: ProjectType::Mod,
        explicit: true,
        preferred: ProviderId::Url,
        sources: BTreeMap::from([(
            ProviderId::Url,
            SourceFile {
                url: Some(format!("http://{address}/example.jar")),
                ..Default::default()
            },
        )]),
        ..Default::default()
    };
    save_item(dir.path(), &item).unwrap();
    server.join().unwrap();
    let items = read_items(dir.path()).unwrap();
    let added = items
        .iter()
        .find(|item| item.name == "Local test download")
        .unwrap();
    assert!(added.file.ends_with(".pw.toml"));
    assert_eq!(added.active().unwrap().filename, "example.jar");
    let data = std::fs::read_to_string(dir.path().join(&added.file)).unwrap();
    assert!(data.contains(&hex::encode(Sha256::digest(b"example"))));
}
