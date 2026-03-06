use anyhow::Result;
use directories::ProjectDirs;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const TEMP_EXTENSIONS: &[&str] = &["download", "partial", "tmp"];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CleanupStats {
    pub files_removed: u64,
    pub model_files_removed: u64,
    pub bytes_reclaimed: u64,
}

pub fn run_startup_cleanup(current_model_path: &str) -> Result<CleanupStats> {
    let current_model = normalize_path(current_model_path);
    let roots = managed_storage_roots();
    let stats = prune_managed_storage(roots.as_slice(), current_model.as_deref())?;

    if stats.files_removed > 0 {
        tracing::info!(
            files_removed = stats.files_removed,
            model_files_removed = stats.model_files_removed,
            bytes_reclaimed = stats.bytes_reclaimed,
            "startup cleanup removed stale Shard artifacts"
        );
    }

    Ok(stats)
}

fn managed_storage_roots() -> Vec<PathBuf> {
    if let Ok(raw) = std::env::var("SHARD_CLEANUP_ROOTS") {
        let roots = raw
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if !roots.is_empty() {
            return dedupe_paths(roots);
        }
    }

    let mut roots = Vec::new();

    if let Some(project_dirs) = ProjectDirs::from("com", "shard", "Shard") {
        roots.push(project_dirs.data_dir().to_path_buf());
        roots.push(project_dirs.data_local_dir().to_path_buf());
    }

    if let Some(local_dir) = dirs::data_local_dir() {
        roots.push(local_dir.join("shard"));
        roots.push(local_dir.join("Shard"));
    }

    if let Some(data_dir) = dirs::data_dir() {
        roots.push(data_dir.join("shard"));
        roots.push(data_dir.join("Shard"));
    }

    dedupe_paths(roots)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if let Some(key) = path_key(path.as_path()) {
            if seen.insert(key) {
                deduped.push(path);
            }
        } else {
            deduped.push(path);
        }
    }
    deduped
}

fn prune_managed_storage(roots: &[PathBuf], current_model: Option<&Path>) -> Result<CleanupStats> {
    let current_key = current_model.and_then(path_key);
    let mut stats = CleanupStats::default();

    for root in roots {
        if !root.exists() {
            continue;
        }
        prune_dir(
            root.as_path(),
            current_key.as_deref(),
            current_model.is_some(),
            &mut stats,
        )?;
        let _ = prune_empty_dirs(root.as_path())?;
    }

    Ok(stats)
}

fn prune_dir(
    dir: &Path,
    current_key: Option<&str>,
    prune_models: bool,
    stats: &mut CleanupStats,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            prune_dir(path.as_path(), current_key, prune_models, stats)?;
            continue;
        }

        if should_remove(path.as_path(), current_key, prune_models) {
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            fs::remove_file(path.as_path())?;
            stats.files_removed += 1;
            stats.bytes_reclaimed += size;
            if is_managed_model_file(path.as_path()) {
                stats.model_files_removed += 1;
            }
        }
    }

    Ok(())
}

fn should_remove(path: &Path, current_key: Option<&str>, prune_models: bool) -> bool {
    if is_temp_file(path) {
        return true;
    }

    if !prune_models || !is_managed_model_file(path) {
        return false;
    }

    match (path_key(path), current_key) {
        (Some(candidate), Some(current)) => candidate != current,
        _ => true,
    }
}

fn is_temp_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            TEMP_EXTENSIONS
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
        .unwrap_or(false)
}

fn is_managed_model_file(path: &Path) -> bool {
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false)
    {
        return false;
    }

    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|segment| segment.eq_ignore_ascii_case("models"))
            .unwrap_or(false)
    })
}

fn prune_empty_dirs(dir: &Path) -> Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }

    let mut is_empty = true;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if !prune_empty_dirs(path.as_path())? {
                is_empty = false;
            }
        } else {
            is_empty = false;
        }
    }

    if is_empty {
        fs::remove_dir(dir)?;
        return Ok(true);
    }

    Ok(false)
}

fn normalize_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = PathBuf::from(trimmed);
    if candidate.exists() {
        return fs::canonicalize(candidate.as_path())
            .ok()
            .or(Some(candidate));
    }

    if candidate.is_absolute() {
        return Some(candidate);
    }

    std::env::current_dir().ok().map(|cwd| cwd.join(candidate))
}

fn path_key(path: &Path) -> Option<String> {
    let normalized = if path.exists() {
        fs::canonicalize(path)
            .ok()
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    let rendered = normalized.to_string_lossy().trim().to_string();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::{prune_managed_storage, CleanupStats};
    use anyhow::Result;
    use std::path::PathBuf;

    fn unique_temp_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time ok")
            .as_nanos();
        dir.push(format!(
            "shard-gui-cleanup-{label}-{}-{ts}",
            std::process::id()
        ));
        dir
    }

    #[test]
    fn prune_managed_storage_removes_temps_and_old_models() -> Result<()> {
        let root = unique_temp_dir("remove-old-models");
        let models_dir = root.join("models").join("meta-llama").join("1.0.0");
        std::fs::create_dir_all(models_dir.as_path())?;

        let active_model = models_dir.join("active.gguf");
        let stale_model = models_dir.join("stale.gguf");
        let partial = models_dir.join("stale.gguf.download");

        std::fs::write(active_model.as_path(), b"active")?;
        std::fs::write(stale_model.as_path(), b"stale")?;
        std::fs::write(partial.as_path(), b"partial")?;

        let stats = prune_managed_storage(std::slice::from_ref(&root), Some(active_model.as_path()))?;

        assert_eq!(
            stats,
            CleanupStats {
                files_removed: 2,
                model_files_removed: 1,
                bytes_reclaimed: 12,
            }
        );
        assert!(active_model.exists());
        assert!(!stale_model.exists());
        assert!(!partial.exists());

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn prune_managed_storage_keeps_models_when_current_is_unknown() -> Result<()> {
        let root = unique_temp_dir("keep-unknown-models");
        let models_dir = root.join("models").join("meta-llama").join("1.0.0");
        std::fs::create_dir_all(models_dir.as_path())?;

        let model = models_dir.join("kept.gguf");
        let partial = models_dir.join("kept.gguf.download");

        std::fs::write(model.as_path(), b"model")?;
        std::fs::write(partial.as_path(), b"partial")?;

        let stats = prune_managed_storage(std::slice::from_ref(&root), None)?;

        assert_eq!(
            stats,
            CleanupStats {
                files_removed: 1,
                model_files_removed: 0,
                bytes_reclaimed: 7,
            }
        );
        assert!(model.exists());
        assert!(!partial.exists());

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }
}
