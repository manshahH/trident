use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{App, Manager, Runtime};

use crate::error::{AppError, Result};

pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    pub fn for_app<R: Runtime>(app: &App<R>) -> Result<Self> {
        let root = app
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Io(error.to_string()))?;
        Ok(Self::from_root(root))
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.log_dir())?;
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database_path(&self) -> PathBuf {
        self.root.join("trident.db")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::AppPaths;

    #[test]
    fn creates_the_data_and_log_directories() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let paths = AppPaths::from_root(directory.path().join("Trident"));

        paths.ensure_directories()?;

        assert!(paths.root().is_dir());
        assert!(paths.log_dir().is_dir());
        assert_eq!(paths.database_path(), paths.root().join("trident.db"));
        Ok(())
    }

    #[test]
    fn reports_an_io_error_when_the_data_path_is_a_file() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let data_path = directory.path().join("Trident");
        fs::write(&data_path, [])?;
        let paths = AppPaths::from_root(data_path);

        let result = paths.ensure_directories();

        assert!(matches!(result, Err(crate::error::AppError::Io(_))));
        Ok(())
    }
}
