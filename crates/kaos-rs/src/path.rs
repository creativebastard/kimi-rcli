//! Path abstraction for async file operations.

use crate::error::{KaosError, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// An async-aware path abstraction.
///
/// `KaosPath` wraps a `PathBuf` and provides async methods for file operations.
/// It is designed to be used with Tokio's async runtime.
///
/// # Examples
///
/// ```
/// use kaos_rs::KaosPath;
///
/// # async fn example() -> kaos_rs::Result<()> {
/// let path = KaosPath::cwd().join("example.txt");
/// if path.exists().await {
///     let content = path.read_file().await?;
///     println!("Content: {}", content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KaosPath {
    inner: PathBuf,
}

impl KaosPath {
    /// Creates a new `KaosPath` from a path-like type.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    /// use std::path::PathBuf;
    ///
    /// let path = KaosPath::from(PathBuf::from("/tmp"));
    /// ```
    pub fn from<P: AsRef<Path>>(path: P) -> Self {
        Self {
            inner: path.as_ref().to_path_buf(),
        }
    }

    /// Reads the entire file contents as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist
    /// - The file cannot be read
    /// - The file contents are not valid UTF-8
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let path = KaosPath::from("/tmp/example.txt");
    /// match path.read_file().await {
    ///     Ok(content) => println!("File content: {}", content),
    ///     Err(e) => eprintln!("Error reading file: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_file(&self) -> Result<String> {
        tokio::fs::read_to_string(&self.inner)
            .await
            .map_err(KaosError::from)
    }

    /// Writes the given content to the file, creating it if necessary.
    ///
    /// If the file already exists, it will be truncated. Parent directories
    /// are not automatically created.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The parent directory does not exist
    /// - The file cannot be written
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let path = KaosPath::from("/tmp/example.txt");
    /// path.write_file("Hello, World!").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_file(&self, content: &str) -> Result<()> {
        tokio::fs::write(&self.inner, content)
            .await
            .map_err(KaosError::from)
    }

    /// Reads the contents of a directory.
    ///
    /// Returns a vector of `KaosPath` representing the entries in the directory.
    /// The entries are not sorted and include both files and directories.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path does not exist
    /// - The path is not a directory
    /// - The directory cannot be read
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let dir = KaosPath::cwd();
    /// let entries = dir.read_dir().await?;
    /// for entry in entries {
    ///     println!("Entry: {:?}", entry);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_dir(&self) -> Result<Vec<KaosPath>> {
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&self.inner).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            entries.push(KaosPath::from(entry.path()));
        }

        Ok(entries)
    }

    /// Checks if the path exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() {
    /// let path = KaosPath::from("/tmp");
    /// if path.exists().await {
    ///     println!("Path exists");
    /// }
    /// # }
    /// ```
    pub async fn exists(&self) -> bool {
        tokio::fs::try_exists(&self.inner).await.unwrap_or(false)
    }

    /// Checks if the path is a file.
    ///
    /// Returns `false` if the path does not exist or is not a file.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() {
    /// let path = KaosPath::from("/tmp/example.txt");
    /// if path.is_file().await {
    ///     println!("It's a file");
    /// }
    /// # }
    /// ```
    pub async fn is_file(&self) -> bool {
        match tokio::fs::metadata(&self.inner).await {
            Ok(metadata) => metadata.is_file(),
            Err(_) => false,
        }
    }

    /// Checks if the path is a directory.
    ///
    /// Returns `false` if the path does not exist or is not a directory.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() {
    /// let path = KaosPath::from("/tmp");
    /// if path.is_dir().await {
    ///     println!("It's a directory");
    /// }
    /// # }
    /// ```
    pub async fn is_dir(&self) -> bool {
        match tokio::fs::metadata(&self.inner).await {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => false,
        }
    }

    /// Joins this path with another path.
    ///
    /// Returns a new `KaosPath` with the joined path.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// let base = KaosPath::from("/tmp");
    /// let file = base.join("example.txt");
    /// assert_eq!(file.as_path(), std::path::Path::new("/tmp/example.txt"));
    /// ```
    pub fn join(&self, path: impl AsRef<Path>) -> KaosPath {
        KaosPath::from(self.inner.join(path))
    }

    /// Returns the parent directory of this path.
    ///
    /// Returns `None` if this path is the root or has no parent.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// let path = KaosPath::from("/tmp/example.txt");
    /// let parent = path.parent();
    /// assert!(parent.is_some());
    /// assert_eq!(parent.unwrap().as_path(), std::path::Path::new("/tmp"));
    /// ```
    pub fn parent(&self) -> Option<KaosPath> {
        self.inner.parent().map(KaosPath::from)
    }

    /// Returns the file name of this path.
    ///
    /// Returns `None` if this path ends in `..` or is the root.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    /// use std::ffi::OsStr;
    ///
    /// let path = KaosPath::from("/tmp/example.txt");
    /// assert_eq!(path.file_name(), Some(OsStr::new("example.txt")));
    /// ```
    pub fn file_name(&self) -> Option<&OsStr> {
        self.inner.file_name()
    }

    /// Returns the canonical, absolute form of the path.
    ///
    /// Resolves all symbolic links and normalizes the path.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path does not exist
    /// - A symbolic link in the path cannot be resolved
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let path = KaosPath::from("/tmp/../tmp/example.txt");
    /// let canonical = path.canonicalize().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn canonicalize(&self) -> Result<KaosPath> {
        tokio::fs::canonicalize(&self.inner)
            .await
            .map(KaosPath::from)
            .map_err(KaosError::from)
    }

    /// Returns the current working directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be determined.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// let cwd = KaosPath::cwd();
    /// println!("Current directory: {:?}", cwd.as_path());
    /// ```
    pub fn cwd() -> KaosPath {
        KaosPath::from(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Returns the user's home directory.
    ///
    /// Returns the current directory if the home directory cannot be determined.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// let home = KaosPath::home();
    /// println!("Home directory: {:?}", home.as_path());
    /// ```
    pub fn home() -> KaosPath {
        KaosPath::from(
            dirs::home_dir()
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from(".")),
        )
    }

    /// Returns the underlying `Path` reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    ///
    /// let path = KaosPath::from("/tmp/example.txt");
    /// assert_eq!(path.as_path(), std::path::Path::new("/tmp/example.txt"));
    /// ```
    pub fn as_path(&self) -> &Path {
        &self.inner
    }

    /// Returns the underlying `PathBuf`.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::KaosPath;
    /// use std::path::PathBuf;
    ///
    /// let path = KaosPath::from("/tmp/example.txt");
    /// let path_buf: PathBuf = path.into_path_buf();
    /// ```
    pub fn into_path_buf(self) -> PathBuf {
        self.inner
    }
}

impl AsRef<Path> for KaosPath {
    fn as_ref(&self) -> &Path {
        &self.inner
    }
}

impl From<PathBuf> for KaosPath {
    fn from(path: PathBuf) -> Self {
        Self { inner: path }
    }
}

impl From<&Path> for KaosPath {
    fn from(path: &Path) -> Self {
        Self {
            inner: path.to_path_buf(),
        }
    }
}

impl From<&str> for KaosPath {
    fn from(path: &str) -> Self {
        Self {
            inner: PathBuf::from(path),
        }
    }
}

impl From<String> for KaosPath {
    fn from(path: String) -> Self {
        Self {
            inner: PathBuf::from(path),
        }
    }
}

impl std::fmt::Display for KaosPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.display().fmt(f)
    }
}

// Helper module for home directory detection
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    let home_drive = std::env::var("HOMEDRIVE").ok()?;
                    let home_path = std::env::var("HOMEPATH").ok()?;
                    Some(PathBuf::from(home_drive).join(home_path))
                })
        }
    }
}
