use anyhow::Result;
use serde_json::json;
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::instance;
use crate::lockfile;
use crate::manifest::{self, Manifest};
use crate::packignore::ExportFilter;
use crate::ptype::Folders;

// Packdesk does not bundle Packweave's self-updating launcher mod. Native
// packwiz packs use packwiz-installer instead; these names are retained only so
// legacy project files can be recognised and ignored safely.
pub(crate) const INSTALLER: &[u8] = &[];
pub(crate) const INSTALLER_JAR: &str = "packdesk-installer.jar";
pub(crate) const PACK_JAR: &str = "overrides/packdesk-installer.jar";
pub(crate) const PACK_MOD_JAR: &str = "overrides/mods/packdesk-installer.jar";

fn icon_bytes(pack_dir: &Path) -> Option<Vec<u8>> {
    std::fs::read(instance::overrides_dir(pack_dir).join("icon.png")).ok()
}

pub async fn export_instance(
    pack_dir: &Path,
    out_zip: &Path,
    env: &str,
    loader_version: Option<&str>,
) -> Result<()> {
    let filter = ExportFilter::load(pack_dir);
    let mut manifest = manifest::read(pack_dir)?;
    let folders = Folders::of(&manifest);
    let mut lock = lockfile::read(pack_dir)?;
    lock.mods.retain(|m| !m.disabled && !filter.skips_mod(m));
    if manifest.loader_version.as_deref().unwrap_or("").is_empty() {
        manifest.loader_version = loader_version.map(str::to_string);
    }
    let client = reqwest::Client::builder()
        .user_agent(crate::USER_AGENT)
        .build()?;

    let file = std::fs::File::create(out_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let icon = icon_bytes(pack_dir);
    let key = icon_key(&manifest.name);
    let mut cfg = format!("InstanceType=OneSix\nname={}\n", manifest.name);
    if icon.is_some() {
        cfg.push_str(&format!("iconKey={key}\n"));
    }
    zip.start_file("instance.cfg", options)?;
    zip.write_all(cfg.as_bytes())?;

    if let Some(bytes) = &icon {
        zip.start_file(format!("{key}.png"), options)?;
        zip.write_all(bytes)?;
    }

    zip.start_file("mmc-pack.json", options)?;
    zip.write_all(mmc_pack(&manifest)?.as_bytes())?;

    for m in &lock.mods {
        if m.unsupported_on(env) {
            continue;
        }
        let src = match m.active() {
            Some(s) if !s.download_url.is_empty() && !m.no_bundle => s,
            _ => continue,
        };
        let bytes = crate::util::download(&client, &src.download_url, &m.name).await?;
        zip.start_file(
            format!(
                ".minecraft/{}/{}",
                folders.install_dir(m.project_type),
                src.filename
            ),
            options,
        )?;
        zip.write_all(&bytes)?;
    }

    for file in filter.overrides(pack_dir) {
        let side = crate::instance::override_side(&manifest, &file.rel, &file.path);
        if !crate::instance::side_allows(&side, env) {
            continue;
        }
        zip.start_file(format!(".minecraft/{}", file.rel), options)?;
        zip.write_all(&std::fs::read(&file.path)?)?;
    }

    zip.finish()?;
    Ok(())
}

pub(crate) fn mmc_pack(manifest: &Manifest) -> Result<String> {
    let mut components = vec![json!({
        "uid": "net.minecraft",
        "version": manifest.minecraft,
        "important": true,
    })];
    if matches!(manifest.loader.as_str(), "fabric" | "quilt") {
        components.push(json!({
            "uid": "net.fabricmc.intermediary",
            "version": manifest.minecraft,
            "dependencyOnly": true,
        }));
    }
    if let Some(uid) = loader_uid(&manifest.loader) {
        components.push(json!({
            "uid": uid,
            "version": manifest.loader_version.clone().unwrap_or_default(),
        }));
    }
    Ok(serde_json::to_string_pretty(&json!({
        "components": components,
        "formatVersion": 1,
    }))?)
}

pub(crate) fn loader_uid(loader: &str) -> Option<&'static str> {
    match loader {
        "fabric" => Some("net.fabricmc.fabric-loader"),
        "quilt" => Some("org.quiltmc.quilt-loader"),
        "forge" => Some("net.minecraftforge"),
        "neoforge" => Some("net.neoforged"),
        _ => None,
    }
}

fn icon_key(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if s.is_empty() {
        "modpack".into()
    } else {
        s
    }
}
