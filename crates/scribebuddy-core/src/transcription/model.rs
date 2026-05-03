use crate::ModelSize;
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new() -> Result<Self> {
        let models_dir = dirs_app_models()?;
        std::fs::create_dir_all(&models_dir)
            .context("Failed to create models directory")?;
        Ok(Self { models_dir })
    }

    pub fn model_path(&self, size: &ModelSize, multilingual: bool) -> PathBuf {
        self.models_dir.join(size.filename(multilingual))
    }

    pub fn is_model_available(&self, size: &ModelSize, multilingual: bool) -> bool {
        self.model_path(size, multilingual).exists()
    }

    pub fn download_model(
        &self,
        size: &ModelSize,
        multilingual: bool,
        progress_callback: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<()> {
        let url = size.download_url(multilingual);
        let dest = self.model_path(size, multilingual);

        if dest.exists() {
            log::info!("Model already exists at {:?}", dest);
            return Ok(());
        }

        log::info!("Downloading model from {} to {:?}", url, dest);

        let response = ureq::get(&url)
            .call()
            .context("Failed to start model download")?;

        let total_size: u64 = response
            .header("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut reader = response.into_reader();
        let mut file = std::fs::File::create(&dest)
            .context("Failed to create model file")?;
        let mut buf = [0u8; 8192];
        let mut downloaded: u64 = 0;

        loop {
            let n = reader.read(&mut buf)
                .context("Failed to read download stream")?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .context("Failed to write model data")?;
            downloaded += n as u64;
            progress_callback(downloaded, total_size);
        }

        file.flush().context("Failed to flush model file")?;

        log::info!("Model downloaded: {:.1} MB", downloaded as f64 / 1_048_576.0);
        Ok(())
    }
}

fn dirs_app_models() -> Result<PathBuf> {
    let base = dirs_base()?;
    Ok(base.join("models"))
}

fn dirs_base() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
        })
        .context("Could not determine home directory")?;

    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("ScribeBuddy"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path_english() {
        let mgr = ModelManager::new().unwrap();
        let path = mgr.model_path(&ModelSize::Small, false);
        assert!(
            path.to_string_lossy().ends_with("ggml-small.en.bin"),
            "path: {:?}",
            path
        );
    }

    #[test]
    fn test_model_path_multilingual() {
        let mgr = ModelManager::new().unwrap();
        let path = mgr.model_path(&ModelSize::Small, true);
        assert!(
            path.to_string_lossy().ends_with("ggml-small.bin"),
            "path: {:?}",
            path
        );
        // Must NOT be the .en variant
        assert!(!path.to_string_lossy().ends_with(".en.bin"), "path: {:?}", path);
    }

    #[test]
    fn test_model_path_large_always_multilingual() {
        let mgr = ModelManager::new().unwrap();
        // Large is always multilingual regardless of the flag
        let en_path = mgr.model_path(&ModelSize::Large, false);
        let ml_path = mgr.model_path(&ModelSize::Large, true);
        assert!(en_path.to_string_lossy().ends_with("ggml-large-v3.bin"));
        assert_eq!(en_path, ml_path);
    }

    #[test]
    fn test_model_not_available_when_missing() {
        let mgr = ModelManager::new().unwrap();
        // Use Large multilingual — almost certainly not downloaded in test env
        // (we only check the logic, not the actual download)
        let path = mgr.model_path(&ModelSize::Large, true);
        let available = mgr.is_model_available(&ModelSize::Large, true);
        // Path must point to the correct file regardless of whether it exists
        assert!(path.to_string_lossy().ends_with("ggml-large-v3.bin"));
        // If the file doesn't exist, is_model_available must return false
        if !path.exists() {
            assert!(!available);
        }
    }
}
