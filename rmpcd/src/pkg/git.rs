use std::{ffi::OsStr, path::Path, process::Output};

use anyhow::{Context, Result};
use tokio::process::Command;

pub struct Git {
    command: Command,
}

impl Git {
    pub async fn clone(url: &str, path: &Path, branch: Option<&str>) -> Result<()> {
        let mut git = Self::default();
        git.arg("clone");
        if let Some(branch) = branch {
            git.args(["-b", branch]);
        }
        git.args(["--", url]).arg(path);
        git.exec().await
    }

    pub async fn fetch(repo_dir: &Path, branch: Option<&str>) -> Result<()> {
        let mut git = Self::default();
        git.arg("fetch").current_dir(repo_dir);
        if let Some(branch) = branch {
            git.args(["origin", branch]);
        }
        git.exec().await
    }

    pub async fn checkout(repo_dir: &Path, rev: &str) -> Result<()> {
        let mut git = Self::default();
        git.args(["checkout", rev, "--force"]).current_dir(repo_dir);
        git.exec().await
    }

    pub async fn rev(repo_dir: &Path, arg: &str) -> Result<String> {
        let mut git = Self::default();
        git.args(["rev-parse", arg]).current_dir(repo_dir);

        let mut rev =
            String::from_utf8(git.output().await.context("Failed to get git revision")?.stdout)
                .context("Failed to parse git revision as UTF-8")?;

        if rev.ends_with('\n') {
            rev.pop();
        }

        Ok(rev)
    }

    pub async fn default_branch(repo_dir: &Path) -> Result<String> {
        let mut git = Self::default();
        git.args(["remote", "set-head", "origin", "-a"]).current_dir(repo_dir);
        git.exec().await.context("Failed to set remote HEAD")?;

        let mut git = Self::default();
        git.args(["symbolic-ref", "refs/remotes/origin/HEAD"]).current_dir(repo_dir);

        let result = git.output().await.context("Failed to get default branch")?;
        let result =
            String::from_utf8(result.stdout).context("Failed to parse default branch as UTF-8")?;

        result.trim().strip_prefix("refs/remotes/origin/").map_or_else(
            || Err(anyhow::anyhow!("Failed to parse default branch from git output: {result}")),
            |s| Ok(s.to_string()),
        )
    }

    fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.command.arg(arg);
        self
    }

    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    async fn output(mut self) -> Result<Output> {
        tracing::trace!("Executing git command: {:?}", self.command);
        let result = self.command.output().await.context("Failed to execute git command")?;

        if !result.status.success() {
            tracing::error!(
                stdout = ?String::from_utf8_lossy(&result.stdout),
                stderr = ?String::from_utf8_lossy(&result.stderr),
                "Git command failed with exit code",
            );
            anyhow::bail!("Git command failed with exit code: {:?}", result.status.code());
        }

        Ok(result)
    }

    async fn exec(mut self) -> Result<()> {
        tracing::trace!("Executing git command: {:?}", self.command);
        let result = self.command.output().await.context("Failed to execute git command")?;

        if !result.status.success() {
            tracing::error!(
                stdout = ?String::from_utf8_lossy(&result.stdout),
                stderr = ?String::from_utf8_lossy(&result.stderr),
                "Git command failed with exit code",
            );
            anyhow::bail!("Git command failed with exit code: {:?}", result.status.code());
        }

        Ok(())
    }
}

impl Default for Git {
    fn default() -> Self {
        let mut command = Command::new("git");
        command.kill_on_drop(true);
        command.args([
            "-c",
            "advice.detachedHead=false",
            "-c",
            "core.eol=lf",
            "-c",
            "core.autocrlf=false",
            "-c",
            "checkout.defaultRemote=origin",
            "-c",
            "clone.defaultRemoteName=origin",
        ]);

        Self { command }
    }
}
