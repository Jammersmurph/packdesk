use std::path::{Path, PathBuf};
use tauri::State;

use crate::cfpack;
use crate::curseforge::CurseForge;
use crate::dist;
use crate::lockfile::{self, Lockfile};
use crate::manifest::{self, Manifest};
use crate::modrinth::Modrinth;
use crate::mrpack;
use crate::packignore::{ExportFilter, PackFile};
use crate::ptype::Folders;
use crate::publish::{self, Bundling};
use crate::secrets;

use super::{blocking, es, resolve_loader_version};

#[tauri::command]
pub async fn export_mrpack(
    mr: State<'_, Modrinth>,
    path: String,
    output: String,
    env: String,
) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if crate::native::is_native(&dir) {
        if env != "client" {
            return Err("packwiz's Modrinth export does not support a server-only target. Use CurseForge export with the server side.".into());
        }
        let _held = super::lock_pack(&dir).await;
        return blocking(move || {
            crate::native::run(
                &dir,
                &[
                    "modrinth".into(),
                    "export".into(),
                    "--output".into(),
                    output,
                ],
            )
            .map(|_| ())
        })
        .await;
    }
    let filter = ExportFilter::load(&dir);
    let mut lock = lockfile::read(&dir).map_err(es)?;
    lock.mods.retain(|m| !m.disabled && !filter.skips_mod(m));
    let manifest = manifest::read(&dir).map_err(es)?;
    lock.loader_version = resolve_loader_version(&mr, &manifest).await;
    mrpack::export(
        &lock,
        &manifest,
        &manifest.version,
        &PathBuf::from(output),
        &env,
        &filter.overrides(&dir),
    )
    .await
    .map_err(es)
}

#[tauri::command]
pub async fn export_curseforge(
    mr: State<'_, Modrinth>,
    cf: State<'_, CurseForge>,
    path: String,
    output: String,
    env: String,
) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if crate::native::is_native(&dir) {
        let _held = super::lock_pack(&dir).await;
        return blocking(move || {
            crate::native::run(
                &dir,
                &[
                    "curseforge".into(),
                    "export".into(),
                    "--side".into(),
                    env,
                    "--output".into(),
                    output,
                ],
            )
            .map(|_| ())
        })
        .await;
    }
    let filter = ExportFilter::load(&dir);
    let mut lock = lockfile::read(&dir).map_err(es)?;
    lock.mods.retain(|m| !m.disabled && !filter.skips_mod(m));
    let manifest = manifest::read(&dir).map_err(es)?;
    lock.loader_version = resolve_loader_version(&mr, &manifest).await;
    cfpack::export(
        &cf,
        &lock,
        &manifest,
        &manifest.version,
        &PathBuf::from(output),
        &env,
        &filter.overrides(&dir),
    )
    .await
    .map_err(es)
}

#[tauri::command]
pub async fn export_instance(
    mr: State<'_, Modrinth>,
    path: String,
    output: String,
    env: String,
) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    if crate::native::is_native(&dir) {
        return Err("Prism instance exports for native packwiz packs are not available yet. Export a .mrpack and import it in Prism.".into());
    }
    let manifest = manifest::read(&dir).map_err(es)?;
    let lv = resolve_loader_version(&mr, &manifest).await;
    dist::export_instance(&dir, &PathBuf::from(output), &env, lv.as_deref())
        .await
        .map_err(es)
}

#[tauri::command]
pub async fn release_folder() -> Result<String, String> {
    super::blocking(|| {
        let root = std::env::temp_dir().join("packweave-release");
        for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age.as_secs() > 3600);
            if stale {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let folder = root.join(crate::cache::now().to_string());
        std::fs::create_dir_all(&folder)?;
        Ok(folder.to_string_lossy().to_string())
    })
    .await
}

#[tauri::command]
pub async fn release_checkout(path: String, commit: String) -> Result<String, String> {
    blocking(move || {
        crate::worktree::checkout_commit(&PathBuf::from(&path), &commit)
            .map(|folder| folder.to_string_lossy().to_string())
    })
    .await
}

#[tauri::command]
pub async fn release_checkout_remove(path: String, folder: String) -> Result<(), String> {
    blocking(move || crate::worktree::remove_build(&PathBuf::from(&path), &PathBuf::from(&folder)))
        .await
}

#[tauri::command]
pub async fn include_packwiz_installer(path: String) -> Result<(), String> {
    let native = crate::native::is_native(Path::new(&path));
    let _held = super::lock_pack(&PathBuf::from(&path)).await;
    blocking(move || {
        if native {
            crate::native::include_installer(&PathBuf::from(&path))
        } else {
            Err(anyhow::anyhow!(
                "Legacy Packweave packs are not supported by Packdesk."
            ))
        }
    })
    .await
}

#[tauri::command]
pub async fn export_mrpack_selfupdate(
    mr: State<'_, Modrinth>,
    path: String,
    output: String,
    url: String,
) -> Result<(), String> {
    let _ = (mr, path, output, url);
    Err(
        "Packdesk does not bundle a game-side updater. Use a regular Modrinth export instead."
            .into(),
    )
}

#[tauri::command]
pub async fn export_curseforge_selfupdate(
    mr: State<'_, Modrinth>,
    path: String,
    output: String,
    url: String,
) -> Result<(), String> {
    let _ = (mr, path, output, url);
    Err(
        "Packdesk does not bundle a game-side updater. Use a regular CurseForge export instead."
            .into(),
    )
}

#[tauri::command]
pub async fn export_dist(
    mr: State<'_, Modrinth>,
    path: String,
    output: String,
    pack_url: String,
) -> Result<(), String> {
    let _ = (mr, path, output, pack_url);
    Err(
        "Packdesk does not bundle a game-side updater. Use a regular instance export instead."
            .into(),
    )
}

fn publish_inputs(dir: &Path) -> Result<(Lockfile, Manifest, Vec<PackFile>), String> {
    if crate::native::is_native(dir) {
        return Err("Publish a packwiz export manually for now. One-click publishing is not available for native packwiz packs yet.".into());
    }
    let filter = ExportFilter::load(dir);
    let mut lock = lockfile::read(dir).map_err(es)?;
    lock.mods.retain(|m| !m.disabled && !filter.skips_mod(m));
    let manifest = manifest::read(dir).map_err(es)?;
    let overrides = filter.overrides(dir);
    Ok((lock, manifest, overrides))
}

#[tauri::command]
pub async fn publish_bundling(
    cf: State<'_, CurseForge>,
    target: String,
    path: String,
    env: String,
) -> Result<Bundling, String> {
    let (lock, manifest, _) = publish_inputs(Path::new(&path))?;
    match target.as_str() {
        "curseforge" => cfpack::bundling(&cf, &lock, &env, &Folders::of(&manifest))
            .await
            .map_err(es),
        _ => Ok(mrpack::bundling(&lock, &env)),
    }
}

#[tauri::command]
pub async fn publish_pack(
    cf: State<'_, CurseForge>,
    target: String,
    path: String,
    project_id: String,
    version_number: String,
    changelog: String,
    env: String,
) -> Result<String, String> {
    let (lock, manifest, overrides) = publish_inputs(Path::new(&path))?;
    match target.as_str() {
        "curseforge" => {
            let token = secrets::get("curseforge_token").unwrap_or_default();
            publish::publish_curseforge(
                &cf,
                &lock,
                &manifest,
                &overrides,
                &project_id,
                &version_number,
                &changelog,
                &env,
                &token,
            )
            .await
            .map_err(es)
        }
        _ => {
            let token = secrets::get("modrinth_token").unwrap_or_default();
            publish::publish_modrinth(
                &lock,
                &manifest,
                &overrides,
                &project_id,
                &version_number,
                &changelog,
                &env,
                &token,
            )
            .await
            .map_err(es)
        }
    }
}
