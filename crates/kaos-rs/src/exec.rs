//! Process execution for async command running.

use crate::error::{KaosError, Result};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};

/// The output of a completed process.
///
/// Contains the exit status, stdout, and stderr of the process.
#[derive(Debug, Clone)]
pub struct Output {
    /// The exit status of the process.
    pub status: std::process::ExitStatus,
    /// The stdout output, as raw bytes.
    pub stdout: Vec<u8>,
    /// The stderr output, as raw bytes.
    pub stderr: Vec<u8>,
}

impl Output {
    /// Checks if the process exited successfully.
    ///
    /// Returns `true` if the exit code was 0.
    pub fn success(&self) -> bool {
        self.status.success()
    }

    /// Returns the stdout as a string, if valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the stdout is not valid UTF-8.
    pub fn stdout_str(&self) -> std::result::Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.stdout.clone())
    }

    /// Returns the stderr as a string, if valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the stderr is not valid UTF-8.
    pub fn stderr_str(&self) -> std::result::Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.stderr.clone())
    }
}

/// A running process with access to its I/O streams.
///
/// This struct represents a spawned process that is currently running.
/// It provides access to the stdin, stdout, and stderr streams.
///
/// # Examples
///
/// ```
/// use kaos_rs::{Command, KaosPath};
///
/// # async fn example() -> kaos_rs::Result<()> {
/// let mut cmd = Command::new("cat");
/// let mut process = cmd.spawn().await?;
/// // Interact with the process...
/// # Ok(())
/// # }
/// ```
pub struct Process {
    /// The stdin stream of the process, if captured.
    pub stdin: Option<ChildStdin>,
    /// The stdout stream of the process, if captured.
    pub stdout: Option<ChildStdout>,
    /// The stderr stream of the process, if captured.
    pub stderr: Option<ChildStderr>,
    inner: tokio::process::Child,
}

impl Process {
    /// Creates a new `Process` from a `tokio::process::Child`.
    fn new(mut child: tokio::process::Child) -> Self {
        Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: child,
        }
    }

    /// Waits for the process to complete and returns its output.
    ///
    /// This method will wait for the process to finish and collect its
    /// stdout and stderr output.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be waited on.
    pub async fn wait_with_output(self) -> Result<Output> {
        let output = self.inner.wait_with_output().await?;
        Ok(Output {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// Waits for the process to complete and returns its exit status.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be waited on.
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        self.inner.wait().await.map_err(KaosError::from)
    }

    /// Attempts to kill the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be killed.
    pub async fn kill(&mut self) -> Result<()> {
        self.inner.kill().await.map_err(KaosError::from)
    }

    /// Returns the process ID, if available.
    pub fn id(&self) -> Option<u32> {
        self.inner.id()
    }
}

impl std::fmt::Debug for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Process")
            .field("id", &self.id())
            .field("has_stdin", &self.stdin.is_some())
            .field("has_stdout", &self.stdout.is_some())
            .field("has_stderr", &self.stderr.is_some())
            .finish()
    }
}

/// A builder for spawning processes.
///
/// `Command` provides a builder-style interface for configuring and
/// spawning processes. It allows setting the program, arguments,
/// environment variables, and working directory.
///
/// # Examples
///
/// ```
/// use kaos_rs::Command;
///
/// # async fn example() -> kaos_rs::Result<()> {
/// let output = Command::new("echo")
///     .arg("Hello, World!")
///     .output()
///     .await?;
///
/// if output.success() {
///     println!("Output: {}", output.stdout_str().unwrap());
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Command {
    program: OsString,
    args: Vec<OsString>,
    env: HashMap<OsString, OsString>,
    current_dir: Option<PathBuf>,
    clear_env: bool,
}

impl Command {
    /// Creates a new `Command` for the given program.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// let cmd = Command::new("ls");
    /// ```
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            env: HashMap::new(),
            current_dir: None,
            clear_env: false,
        }
    }

    /// Adds an argument to the command.
    ///
    /// Multiple arguments can be added by calling this method multiple times.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// let cmd = Command::new("ls")
    ///     .arg("-l")
    ///     .arg("-a");
    /// ```
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// Adds multiple arguments to the command.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// let cmd = Command::new("ls")
    ///     .args(["-l", "-a", "-h"]);
    /// ```
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.args.push(arg.as_ref().to_os_string());
        }
        self
    }

    /// Sets an environment variable for the command.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// let cmd = Command::new("env")
    ///     .env("MY_VAR", "my_value");
    /// ```
    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Self {
        self.env
            .insert(key.as_ref().to_os_string(), val.as_ref().to_os_string());
        self
    }

    /// Sets multiple environment variables for the command.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    /// use std::collections::HashMap;
    ///
    /// let mut env_vars = HashMap::new();
    /// env_vars.insert("VAR1", "value1");
    /// env_vars.insert("VAR2", "value2");
    ///
    /// let cmd = Command::new("env")
    ///     .envs(&env_vars);
    /// ```
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (key, val) in vars {
            self.env
                .insert(key.as_ref().to_os_string(), val.as_ref().to_os_string());
        }
        self
    }

    /// Clears all environment variables for the command.
    ///
    /// After calling this, only environment variables explicitly set
    /// via `env()` or `envs()` will be available to the process.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// let cmd = Command::new("env")
    ///     .env_clear()
    ///     .env("ONLY_VAR", "only_value");
    /// ```
    pub fn env_clear(&mut self) -> &mut Self {
        self.clear_env = true;
        self.env.clear();
        self
    }

    /// Removes an environment variable from the command.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// let cmd = Command::new("env")
    ///     .env_remove("PATH");
    /// ```
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.env.remove(key.as_ref());
        self
    }

    /// Sets the working directory for the command.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    /// use kaos_rs::KaosPath;
    ///
    /// let cmd = Command::new("ls")
    ///     .current_dir(KaosPath::home());
    /// ```
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.current_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Spawns the command as a new process.
    ///
    /// Returns a `Process` that can be used to interact with the running
    /// process. The stdin, stdout, and stderr streams are piped by default.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The program cannot be found
    /// - The program cannot be executed
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let mut process = Command::new("sleep").arg("10").spawn().await?;
    /// // Do something else...
    /// process.kill().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn spawn(&mut self) -> Result<Process> {
        let mut cmd = self.build_tokio_command();
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = cmd.spawn().map_err(KaosError::from)?;
        Ok(Process::new(child))
    }

    /// Runs the command and returns its output.
    ///
    /// This method waits for the process to complete and returns its
    /// stdout and stderr output.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The program cannot be found
    /// - The program cannot be executed
    /// - The output cannot be collected
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let output = Command::new("echo")
    ///     .arg("Hello")
    ///     .output()
    ///     .await?;
    ///
    /// if output.success() {
    ///     println!("{}", output.stdout_str().unwrap());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn output(&mut self) -> Result<Output> {
        let mut cmd = self.build_tokio_command();
        let output = cmd.output().await.map_err(KaosError::from)?;

        Ok(Output {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// Runs the command and returns its status.
    ///
    /// This method waits for the process to complete and returns its
    /// exit status. Stdout and stderr are not captured.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The program cannot be found
    /// - The program cannot be executed
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::Command;
    ///
    /// # async fn example() -> kaos_rs::Result<()> {
    /// let status = Command::new("true")
    ///     .status()
    ///     .await?;
    ///
    /// assert!(status.success());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn status(&mut self) -> Result<std::process::ExitStatus> {
        let mut cmd = self.build_tokio_command();
        cmd.status()
            .await
            .map_err(|e| KaosError::Process(e.to_string()))
    }

    /// Builds a `tokio::process::Command` from this `Command`.
    fn build_tokio_command(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.program);
        cmd.args(&self.args);

        if self.clear_env {
            cmd.env_clear();
        }

        for (key, val) in &self.env {
            cmd.env(key, val);
        }

        if let Some(ref dir) = self.current_dir {
            cmd.current_dir(dir);
        }

        cmd
    }
}

impl Default for Command {
    fn default() -> Self {
        Self::new("")
    }
}

// Re-export Output at the crate level
pub use self::Output as CommandOutput;
