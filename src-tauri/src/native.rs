//! Native packwiz storage and process boundary, added for Packdesk.
//!
//! pack.toml and indexed .pw.toml files are the source of truth. Reading never
//! converts or repairs a pack. Packwiz owns installation and dependency choices.
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::content::ContentItem;
use crate::dist::{INSTALLER, INSTALLER_JAR};
use crate::lockfile::{Lockfile, SourceFile};
use crate::manifest::{Channel, Manifest};
use crate::providers::ProviderId;
use crate::ptype::ProjectType;

pub const PACK_FILE: &str = "pack.toml";

pub fn is_native(dir: &Path) -> bool {
    dir.join(PACK_FILE).is_file()
}

fn read_toml(path: &Path) -> Result<toml::Value> {
    toml::from_str(&std::fs::read_to_string(path)?)
        .with_context(|| format!("Couldn't parse {}", path.display()))
}

fn text<'a>(value: &'a toml::Value, key: &str) -> &'a str {
    value.get(key).and_then(toml::Value::as_str).unwrap_or("")
}

pub fn parse_manifest(source: &str) -> Result<Manifest> {
    let value: toml::Value = toml::from_str(source)?;
    let format = text(&value, "pack-format");
    if !format.starts_with("packwiz:1.") {
        bail!("Unsupported packwiz format {format:?}. Expected packwiz 1.x.");
    }
    let versions = value
        .get("versions")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| anyhow!("pack.toml has no versions table."))?;
    let versions: BTreeMap<String, String> = versions
        .iter()
        .filter_map(|(key, value)| value.as_str().map(|s| (key.clone(), s.into())))
        .collect();
    let minecraft = versions
        .get("minecraft")
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| anyhow!("pack.toml has no Minecraft version."))?;
    let (loader, loader_version) = crate::packwiz::loader_of(&versions);
    let channel = value
        .get("options")
        .and_then(|v| v.get("packdesk-channel"))
        .and_then(toml::Value::as_str)
        .unwrap_or("release");
    Ok(Manifest {
        name: text(&value, "name").into(),
        version: value
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or("1.0.0")
            .into(),
        minecraft,
        loader,
        loader_version,
        channel: match channel {
            "alpha" => Channel::Alpha,
            "beta" => Channel::Beta,
            _ => Channel::Release,
        },
        datapacks: value
            .get("options")
            .and_then(|v| v.get("datapack-folder"))
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        override_sides: None,
        packweave_md: false,
        format_version: 1,
    })
}

pub fn read_manifest(dir: &Path) -> Result<Manifest> {
    parse_manifest(&std::fs::read_to_string(dir.join(PACK_FILE))?)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| anyhow!("No parent directory."))?,
    )?;
    if let Ok(metadata) = std::fs::metadata(path) {
        file.as_file().set_permissions(metadata.permissions())?;
    }
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<()> {
    let path = dir.join(PACK_FILE);
    let mut doc: toml_edit::Document = std::fs::read_to_string(&path)?.parse()?;
    // Editing the document preserves comments and fields unknown to Packdesk.
    doc["name"] = toml_edit::value(&manifest.name);
    doc["version"] = toml_edit::value(&manifest.version);
    doc["versions"]["minecraft"] = toml_edit::value(&manifest.minecraft);
    for loader in ["fabric", "quilt", "forge", "neoforge"] {
        if let Some(table) = doc
            .get_mut("versions")
            .and_then(toml_edit::Item::as_table_mut)
        {
            table.remove(loader);
        }
    }
    if manifest.loader != "vanilla" {
        let version = manifest
            .loader_version
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Choose a loader version before saving."))?;
        doc["versions"][&manifest.loader] = toml_edit::value(version);
    }
    if let Some(folder) = &manifest.datapacks {
        doc["options"]["datapack-folder"] = toml_edit::value(folder);
    }
    doc["options"]["packdesk-channel"] = toml_edit::value(manifest.channel.as_str());
    atomic_write(&path, doc.to_string().as_bytes())
}

pub fn executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("PACKDESK_PACKWIZ") {
        return Ok(path.into());
    }
    let name = if cfg!(windows) {
        "packwiz.exe"
    } else {
        "packwiz"
    };
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let target_name = format!("packwiz-{}{extension}", env!("PACKDESK_BUILD_TARGET"));
    let exe = std::env::current_exe()?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("No executable directory."))?;
    let mut candidates = vec![exe_dir.join(name), exe_dir.join(&target_name)];
    if let Some(parent) = exe_dir.parent() {
        candidates.push(parent.join("lib/packdesk").join(name));
        candidates.push(parent.join("lib/packdesk").join(&target_name));
        candidates.push(parent.join("bin").join(name));
    }
    if let Some(appdir) = std::env::var_os("APPDIR") {
        let appdir = PathBuf::from(appdir);
        candidates.push(appdir.join("usr/bin").join(name));
        candidates.push(appdir.join("usr/lib/packdesk").join(name));
    }
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(&target_name),
    );
    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    // A development build may use an installed CLI; bundled builds always find
    // one of the candidates above.
    Ok(name.into())
}

pub fn run(dir: &Path, args: &[String]) -> Result<String> {
    let output = Command::new(executable()?)
        .current_dir(dir)
        .args(["--yes", "--pack-file", PACK_FILE])
        .args(args)
        .stdin(Stdio::null())
        .env("NO_COLOR", "1")
        .output()
        .context("Couldn't start packwiz. Build the sidecar or set PACKDESK_PACKWIZ.")?;
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        bail!("packwiz {} failed:\n{message}", args.join(" "));
    }
    Ok(message)
}

pub fn refresh(dir: &Path) -> Result<()> {
    run(dir, &["refresh".into()]).map(|_| ())
}

pub fn create(
    dir: &Path,
    name: &str,
    minecraft: &str,
    loader: &str,
    loader_version: Option<&str>,
) -> Result<Manifest> {
    if dir.exists() && std::fs::read_dir(dir)?.next().is_some() {
        bail!("Choose a new or empty folder. Existing packs will not be overwritten.");
    }
    std::fs::create_dir_all(dir)?;
    let loader = if loader == "vanilla" { "none" } else { loader };
    let mut args = vec![
        "init".into(),
        "--name".into(),
        name.into(),
        "--version".into(),
        "1.0.0".into(),
        "--mc-version".into(),
        minecraft.into(),
        "--modloader".into(),
        loader.into(),
    ];
    if loader != "none" {
        if let Some(version) = loader_version.filter(|s| !s.is_empty()) {
            args.extend([format!("--{loader}-version"), version.into()]);
        } else {
            args.push(format!("--{loader}-latest"));
        }
    }
    run(dir, &args)?;
    ensure_ignores(dir)?;
    read_manifest(dir)
}

pub fn ensure_ignores(dir: &Path) -> Result<()> {
    for (filename, patterns) in [
        (
            ".packwizignore",
            &[
                ".pack-local.json",
                ".packdesk/",
                ".packweaveignore",
                ".packweaveunlock",
                "modpack.json",
                "content/",
            ][..],
        ),
        (
            ".gitignore",
            &[".pack-local.json", "*.mrpack", "/*.zip"][..],
        ),
    ] {
        let path = dir.join(filename);
        let mut value = match std::fs::read_to_string(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let missing: Vec<_> = patterns
            .iter()
            .filter(|p| !value.lines().any(|line| line == **p))
            .collect();
        if missing.is_empty() {
            continue;
        }
        if !value.ends_with('\n') {
            value.push('\n');
        }
        value.push_str("\n# Packdesk local metadata\n");
        for pattern in missing {
            value.push_str(pattern);
            value.push('\n');
        }
        atomic_write(&path, value.as_bytes())?;
    }
    Ok(())
}

pub fn safe_path(dir: &Path, rel: &str) -> Result<PathBuf> {
    if Path::new(rel).is_absolute() || rel.starts_with(['/', '\\']) {
        bail!("Path is outside this pack: {rel}");
    }
    let relative = crate::pathsafe::safe_zip_rel(rel)
        .ok_or_else(|| anyhow!("Path is outside this pack: {rel}"))?;
    let path = dir.join(relative);
    let root = dir.canonicalize()?;
    // Check existing ancestors too, preventing an in-pack symlink from escaping.
    let mut ancestor = path.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| anyhow!("Invalid pack path."))?;
    }
    if !ancestor.canonicalize()?.starts_with(root) {
        bail!("Path escapes the pack through a symlink: {rel}");
    }
    Ok(path)
}

fn meta_slug(relative: &str) -> String {
    let file = Path::new(relative)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let trimmed = file
        .strip_suffix(".pw.toml")
        .or_else(|| file.strip_suffix(".toml"))
        .unwrap_or(&file);
    trimmed.to_string()
}

pub fn parse_item(source: &str, relative: &str) -> Result<ContentItem> {
    let value: toml::Value = toml::from_str(source)?;
    let name = text(&value, "name").to_string();
    let filename = text(&value, "filename").to_string();
    let slug = meta_slug(relative);
    let project_type = if relative.contains("resourcepacks/") {
        ProjectType::Resourcepack
    } else if relative.contains("shaderpacks/") {
        ProjectType::Shader
    } else if relative.contains("datapacks/") {
        ProjectType::Datapack
    } else {
        ProjectType::Mod
    };
    let mut base = SourceFile {
        slug,
        filename: filename.clone(),
        version_number: filename,
        ..Default::default()
    };
    if let Some(download) = value.get("download") {
        base.download_url = text(download, "url").into();
        base.url = (!base.download_url.is_empty()).then(|| base.download_url.clone());
        let hash = text(download, "hash").to_string();
        match text(download, "hash-format") {
            "sha1" => base.sha1 = Some(hash),
            "sha512" => base.sha512 = Some(hash),
            _ => {}
        }
    }
    let mut sources = BTreeMap::new();
    if let Some(update) = value.get("update") {
        if let Some(mr) = update.get("modrinth") {
            let mut src = base.clone();
            src.id = Some(text(mr, "mod-id").into());
            src.version_id = text(mr, "version").into();
            sources.insert(ProviderId::Modrinth, src);
        }
        if let Some(cf) = update.get("curseforge") {
            let mut src = base.clone();
            src.id = cf
                .get("project-id")
                .and_then(toml::Value::as_integer)
                .map(|v| v.to_string());
            src.version_id = cf
                .get("file-id")
                .and_then(toml::Value::as_integer)
                .map(|v| v.to_string())
                .unwrap_or_default();
            sources.insert(ProviderId::Curseforge, src);
        }
    }
    let preferred = if sources.contains_key(&ProviderId::Modrinth) {
        ProviderId::Modrinth
    } else if sources.contains_key(&ProviderId::Curseforge) {
        ProviderId::Curseforge
    } else {
        sources.insert(ProviderId::Url, base);
        ProviderId::Url
    };
    let side = text(&value, "side");
    let mut client = "required".to_string();
    let mut server = "required".to_string();
    crate::content::apply_side(side, &mut client, &mut server);
    Ok(ContentItem {
        name,
        file: relative.into(),
        project_type,
        preferred,
        sources,
        side: if side.is_empty() {
            "both".into()
        } else {
            side.into()
        },
        explicit: true,
        pinned: value
            .get("pin")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        client_side: client,
        server_side: server,
        ..Default::default()
    })
}

pub fn index_relative(dir: &Path) -> Result<String> {
    let pack = read_toml(&dir.join(PACK_FILE))?;
    Ok(pack
        .get("index")
        .map(|i| text(i, "file"))
        .filter(|s| !s.is_empty())
        .unwrap_or("index.toml")
        .into())
}

pub fn read_items(dir: &Path) -> Result<Vec<ContentItem>> {
    #[derive(Deserialize)]
    struct Entry {
        file: String,
        #[serde(default)]
        metafile: bool,
    }
    #[derive(Deserialize)]
    struct Index {
        #[serde(default)]
        files: Vec<Entry>,
    }
    let index_path = safe_path(dir, &index_relative(dir)?)?;
    let index: Index = toml::from_str(&std::fs::read_to_string(&index_path)?)?;
    let mut result = Vec::new();
    for entry in index.files.into_iter().filter(|entry| entry.metafile) {
        let parent = index_path.parent().unwrap().strip_prefix(dir)?;
        let relative = parent.join(entry.file).to_string_lossy().replace('\\', "/");
        let path = safe_path(dir, &relative)?;
        result.push(parse_item(&std::fs::read_to_string(path)?, &relative)?);
    }
    result.sort_by_key(|item| item.name.to_lowercase());
    Ok(result)
}

pub fn lockfile(dir: &Path) -> Result<Lockfile> {
    let manifest = read_manifest(dir)?;
    Ok(Lockfile {
        minecraft: manifest.minecraft,
        loader: manifest.loader,
        loader_version: manifest.loader_version,
        mods: read_items(dir)?
            .iter()
            .map(crate::content::to_locked)
            .collect(),
    })
}

pub fn save_item(dir: &Path, item: &ContentItem) -> Result<()> {
    if item.disabled {
        bail!("Disabling native packwiz entries is not supported yet. Remove the mod, or test changes on a Git branch.");
    }
    let source = item
        .active()
        .ok_or_else(|| anyhow!("This item has no active source."))?;
    let existing = read_items(dir)?
        .into_iter()
        .find(|old| crate::content::matches(old, &item.file));
    if let Some(old) = &existing {
        if old.preferred != item.preferred || old.sources.len() != item.sources.len() {
            bail!("Changing alternate sources is not supported for native packwiz packs yet.");
        }
        if old.no_bundle != item.no_bundle || old.explicit != item.explicit {
            bail!("Bundle and dependency-role overrides are not supported for native packwiz packs yet.");
        }
        for (provider, source) in &item.sources {
            if *provider != item.preferred
                && serde_json::to_value(source)? != serde_json::to_value(old.sources.get(provider))?
            {
                bail!("Editing an alternate provider version is not supported for native packwiz packs yet.");
            }
        }
    }
    let old_version = existing
        .as_ref()
        .and_then(ContentItem::active)
        .map(|s| s.version_id.as_str());
    if existing.is_none() || old_version != Some(source.version_id.as_str()) {
        let id = source.id.as_deref().unwrap_or(&source.slug);
        let mut args: Vec<String> = match item.preferred {
            ProviderId::Modrinth => vec![
                "modrinth".into(),
                "add".into(),
                "--project-id".into(),
                id.into(),
            ],
            ProviderId::Curseforge => vec![
                "curseforge".into(),
                "add".into(),
                "--addon-id".into(),
                id.into(),
            ],
            ProviderId::Url => vec![
                "url".into(),
                "add".into(),
                item.name.clone(),
                source.url.as_deref().unwrap_or(&source.download_url).into(),
            ],
            ProviderId::Local => bail!("Use pack files for local content in native packwiz packs."),
        };
        if !source.version_id.is_empty() {
            args.extend([
                if item.preferred == ProviderId::Curseforge {
                    "--file-id"
                } else {
                    "--version-id"
                }
                .into(),
                source.version_id.clone(),
            ]);
        }
        let folder = existing
            .as_ref()
            .and_then(|old| Path::new(&old.file).parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(item.project_type.instance_dir()));
        args.extend([
            "--meta-folder".into(),
            folder.to_string_lossy().into_owned(),
        ]);
        run(dir, &args)?;
        return Ok(());
    }
    let old = existing.unwrap();
    let path = safe_path(dir, &old.file)?;
    let mut doc: toml_edit::Document = std::fs::read_to_string(&path)?.parse()?;
    doc["side"] = toml_edit::value(if item.side == "auto" {
        "both"
    } else {
        &item.side
    });
    doc["pin"] = toml_edit::value(item.pinned);
    atomic_write(&path, doc.to_string().as_bytes())?;
    refresh(dir)
}

pub fn remove(dir: &Path, key: &str) -> Result<()> {
    let item = read_items(dir)?
        .into_iter()
        .find(|item| crate::content::matches(item, key))
        .ok_or_else(|| anyhow!("No packwiz entry matches {key}."))?;
    safe_path(dir, &item.file)?;
    // packwiz resolves remove arguments against metadata basenames, not paths.
    run(dir, &["remove".into(), meta_slug(&item.file)]).map(|_| ())
}

/// Rebuild coupled hashes from the Git index, not the working directory.
///
/// A user can commit one mod while leaving other mod/config changes uncommitted.
/// Hashing the working tree would publish hashes for files absent from the commit.
pub fn stage_index(dir: &Path) -> Result<()> {
    let staged = crate::git::staged_files(dir)?;
    if !staged.iter().any(|(path, _)| path == PACK_FILE) {
        bail!("Include pack.toml in the first commit of a native pack.");
    }
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    let oids: Vec<String> = staged.iter().map(|(_, oid)| oid.clone()).collect();
    let mut write_error = None;
    crate::git::each_blob(dir, &oids, |index, bytes| {
        let result = (|| -> Result<()> {
            let path = safe_path(root, &staged[index].0)?;
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, bytes)?;
            Ok(())
        })();
        if let Err(error) = result {
            write_error = Some(error);
        }
    })?;
    if let Some(error) = write_error {
        return Err(error);
    }
    let index = index_relative(root)?;
    // An initial commit may select the manifest and mods but not the generated
    // index. packwiz refresh accepts an initially empty index.
    let index_path = safe_path(root, &index)?;
    if !index_path.exists() {
        std::fs::create_dir_all(index_path.parent().unwrap())?;
        std::fs::write(&index_path, "")?;
    }
    refresh(root)?;
    crate::git::stage_blob(dir, &index, &std::fs::read(index_path)?)?;
    crate::git::stage_blob(dir, PACK_FILE, &std::fs::read(root.join(PACK_FILE))?)?;
    Ok(())
}

/// Install packwiz-installer into an existing native pack so exports can
/// bootstrap from a self-contained pack URL. packwiz owns the index entry.
pub fn include_installer(pack_dir: &Path) -> Result<()> {
    if !is_native(pack_dir) {
        bail!("This command requires a native packwiz pack.");
    }
    if INSTALLER.is_empty() {
        bail!("Packdesk does not bundle a game-side installer.");
    }
    let path = safe_path(pack_dir, INSTALLER_JAR)?;
    if std::fs::read(&path).ok().as_deref() != Some(INSTALLER) {
        std::fs::write(&path, INSTALLER)?;
    }
    if INSTALLER_JAR != "packwiz-installer.jar" {
        return Ok(());
    }
    run(pack_dir, &["refresh".into()]).map(|_| ())
}

/// Keep these commands before Tauri startup so agents need no display server.
pub fn cli() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first()?.as_str();
    if !["check", "run", "skill", "--help", "-h"].contains(&command) {
        return None;
    }
    let separator = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    let json = args[..separator].iter().any(|arg| arg == "--json");
    let options: Vec<&str> = args[..separator]
        .iter()
        .map(String::as_str)
        .filter(|arg| *arg != "--json")
        .collect();
    let result = (|| -> Result<serde_json::Value> {
        match options.as_slice() {
            ["--help"] | ["-h"] => Ok(serde_json::json!({
                "usage": [
                    "packdesk [open /path/to/pack]",
                    "packdesk check /path/to/pack.toml --json",
                    "packdesk run --pack /path/to/pack.toml --json -- COMMAND ARGS",
                    "packdesk skill install --json"
                ]
            })),
            ["check", path] => {
                let path = Path::new(path);
                let dir = if path.is_file() {
                    path.parent().unwrap()
                } else {
                    path
                };
                Ok(
                    serde_json::json!({ "manifest": read_manifest(dir)?, "lockfile": lockfile(dir)? }),
                )
            }
            ["run", "--pack", path] if separator < args.len() - 1 => {
                let path = Path::new(path);
                let dir = if path.is_file() {
                    path.parent().unwrap()
                } else {
                    path
                };
                let dir = dir.canonicalize()?;
                let output = run(&dir, &args[separator + 1..])?;
                Ok(serde_json::json!({ "ok": true, "output": output }))
            }
            ["skill", "install"] => {
                let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set."))?;
                let dir = PathBuf::from(home).join(".agents/skills/packdesk");
                std::fs::create_dir_all(&dir)?;
                atomic_write(&dir.join("SKILL.md"), include_bytes!("../../SKILL.md"))?;
                Ok(serde_json::json!({ "ok": true, "path": dir }))
            }
            _ => bail!("Invalid arguments. Use --help."),
        }
    })();
    match result {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Some(0)
        }
        Err(error) => {
            if json {
                eprintln!("{}", serde_json::json!({ "error": error.to_string() }));
            } else {
                eprintln!("{error:#}");
            }
            Some(1)
        }
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
