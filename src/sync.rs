use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

pub fn repository() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent()?
        .ancestors()
        .find(|p| p.join("Cargo.toml").is_file() && p.join("scripts").is_dir())
        .map(Path::to_path_buf)
}
pub fn allowed(path: &str) -> bool {
    let p = path.replace('\\', "/");
    let lower = p.to_lowercase();
    if lower.split('/').any(|s| {
        matches!(
            s,
            ".git" | "target" | "publish" | "bin" | "obj" | "work" | "node_modules"
        ) || s.starts_with(".env")
            || s.starts_with("credentials")
            || s.starts_with("secrets")
    }) {
        return false;
    }
    matches!(
        p.as_str(),
        "Cargo.toml"
            | "Cargo.lock"
            | "README.md"
            | "LICENSE"
            | "THIRD_PARTY_NOTICES.md"
            | ".gitignore"
            | ".gitattributes"
    ) || ((p.starts_with("src/") || p.starts_with("tests/")) && p.ends_with(".rs"))
        || ((p.starts_with("scripts/") || p.starts_with("tests/")) && p.ends_with(".ps1"))
}
fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout)
            .trim_end_matches(['\r', '\n'])
            .into())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into())
    }
}
pub fn run_once(repo: &Path) -> Result<(), String> {
    git(repo, &["remote", "get-url", "origin"])?;
    let branch = git(repo, &["branch", "--show-current"])?;
    if branch.is_empty() {
        return Err("Selecione uma branch no Git".into());
    }
    let staged = git(repo, &["diff", "--cached", "--name-only", "-z"])?;
    if staged.split('\0').any(|p| !p.is_empty() && !allowed(p)) {
        return Err("Revise os arquivos já preparados no Git".into());
    }
    let files = git(
        repo,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let mut args = vec!["add", "--all", "--"];
    args.extend(files.split('\0').filter(|p| !p.is_empty() && allowed(p)));
    if args.len() > 3 {
        git(repo, &args)?;
    }
    if !git(repo, &["diff", "--cached", "--name-only"])?.is_empty() {
        git(
            repo,
            &["commit", "-m", "Atualização automática pelo Canto (Rust)"],
        )?;
    }
    git(repo, &["push", "--set-upstream", "origin", &branch])?;
    Ok(())
}
type Snapshot = BTreeMap<PathBuf, (SystemTime, u64)>;
fn snapshot(repo: &Path) -> Snapshot {
    fn visit(repo: &Path, dir: &Path, out: &mut Snapshot) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let Ok(relative) = p.strip_prefix(repo) else {
                continue;
            };
            let s = relative.to_string_lossy().replace('\\', "/");
            if e.file_type().is_ok_and(|t| t.is_symlink()) {
                continue;
            }
            if p.is_dir() {
                if matches!(e.file_name().to_str(), Some("src" | "scripts" | "tests"))
                    || dir != repo
                        && !matches!(
                            e.file_name().to_str(),
                            Some("bin" | "obj" | "target" | ".git")
                        )
                {
                    visit(repo, &p, out);
                }
            } else if allowed(&s) {
                if let Ok(m) = e.metadata() {
                    out.insert(
                        relative.into(),
                        (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()),
                    );
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(repo, repo, &mut out);
    out
}
pub fn stop_old_watcher(repo: &Path) {
    let digest = Sha256::digest(
        repo.to_string_lossy()
            .trim_end_matches('\\')
            .to_lowercase()
            .as_bytes(),
    );
    let key = digest
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let _ = fs::create_dir_all(crate::settings::data_dir());
    let _ = fs::write(
        crate::settings::data_dir().join(format!("sync-{key}.stop")),
        "stop",
    );
}
pub fn start(
    repo: Option<PathBuf>,
    enabled: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
) {
    thread::Builder::new()
        .name("canto-sync".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let Some(repo) = repo else {
                *status.lock().unwrap() = "Abra o app na pasta do projeto para sincronizar".into();
                return;
            };
            stop_old_watcher(&repo);
            let mut before = snapshot(&repo);
            let mut changed = Instant::now();
            let mut pending = true;
            let mut retry = Instant::now();
            let mut was_enabled = false;
            while running.load(Ordering::Relaxed) {
                if enabled.load(Ordering::Relaxed) {
                    if !was_enabled {
                        pending = true;
                        changed = Instant::now();
                    }
                    let now = snapshot(&repo);
                    if now != before {
                        before = now;
                        changed = Instant::now();
                        pending = true;
                        *status.lock().unwrap() =
                            "Alterações detectadas · aguardando salvar".into();
                    }
                    if pending
                        && changed.elapsed() >= Duration::from_secs(20)
                        && Instant::now() >= retry
                    {
                        match run_once(&repo) {
                            Ok(()) => {
                                pending = false;
                                *status.lock().unwrap() = "GitHub sincronizado".into();
                            }
                            Err(e) => {
                                *status.lock().unwrap() = if e.contains("rejected")
                                    || e.contains("fetch first")
                                {
                                    "Há alterações remotas para revisar".into()
                                } else if e.contains("Revise") {
                                    "Revise os arquivos preparados no Git".into()
                                } else if e.contains("origin") || e.contains("not a git repository")
                                {
                                    "Repositório ainda não conectado".into()
                                } else {
                                    "Envio pendente · verifique login e conexão".into()
                                };
                                retry = Instant::now() + Duration::from_secs(60);
                            }
                        }
                    }
                    was_enabled = true;
                } else {
                    *status.lock().unwrap() = "Sincronização pausada".into();
                    was_enabled = false;
                }
                for _ in 0..50 {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        })
        .expect("sync thread");
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_allowlist() {
        for p in ["src/main.rs", "Cargo.lock", "scripts/Build.ps1"] {
            assert!(allowed(p));
        }
        for p in [
            ".env",
            "src/secrets.rs",
            "src/key.pem",
            "target/release/CantoWidget.exe",
            "credentials.json",
        ] {
            assert!(!allowed(p));
        }
    }
    #[test]
    fn real_git_sync_preserves_credentials_and_history() {
        let dir = std::env::temp_dir().join(format!(
            "canto-rust-test-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = dir.join("repo");
        let remote = dir.join("remote.git");
        fs::create_dir_all(&repo).unwrap();
        let g = |root: &Path, args: &[&str]| git(root, args).unwrap();
        g(&dir, &["init", "--bare", remote.to_str().unwrap()]);
        g(&repo, &["init", "-b", "main"]);
        g(&repo, &["config", "user.name", "Test"]);
        g(&repo, &["config", "user.email", "test@example.invalid"]);
        g(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        fs::write(repo.join("README.md"), "one").unwrap();
        fs::write(repo.join(".env"), "TEST=private").unwrap();
        run_once(&repo).unwrap();
        assert_eq!(
            g(&repo, &["ls-tree", "-r", "--name-only", "HEAD"]),
            "README.md"
        );
        fs::write(repo.join("README.md"), "two").unwrap();
        run_once(&repo).unwrap();
        assert_eq!(g(&repo, &["rev-list", "--count", "HEAD"]), "2");
        g(&repo, &["add", ".env"]);
        assert!(run_once(&repo).is_err());
        assert!(dir.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(dir);
    }
}
