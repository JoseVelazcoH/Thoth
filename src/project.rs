use crate::error::ThothError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_WALK_DEPTH: usize = 20;
const CACHE_TTL_SECONDS: i64 = 86400;

pub fn infer_project(directory: &str, conn: &Connection) -> Result<String, ThothError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let cutoff = now - CACHE_TTL_SECONDS;

    let cached: Option<String> = conn
        .query_row(
            "SELECT name FROM projects WHERE path=?1 AND last_seen > ?2",
            rusqlite::params![directory, cutoff],
            |row| row.get(0),
        )
        .ok();

    if let Some(name) = cached {
        return Ok(name);
    }

    Ok(walk_markers(directory))
}

pub fn walk_markers(cwd: &str) -> String {
    let mut current = PathBuf::from(cwd);
    if let Ok(resolved) = current.canonicalize() {
        current = resolved;
    }

    for _ in 0..MAX_WALK_DEPTH {
        if let Some(name) = check_markers(&current) {
            return name;
        }
        let parent = current.parent().map(|p| p.to_path_buf());
        match parent {
            Some(p) if p != current => current = p,
            _ => break,
        }
    }
    String::from("ungrouped")
}

type MarkerFn = fn(&Path) -> Option<String>;

fn check_markers(directory: &Path) -> Option<String> {
    let strategies: &[(&str, MarkerFn)] = &[
        (".git", extract_dirname),
        ("package.json", extract_package_json),
        ("pyproject.toml", extract_pyproject),
        ("Cargo.toml", extract_cargo),
        ("go.mod", extract_gomod),
        ("docker-compose.yml", extract_dirname),
        ("compose.yml", extract_dirname),
    ];

    for &(filename, extractor) in strategies {
        let candidate = directory.join(filename);
        let exists = if filename == ".git" {
            candidate.is_dir()
        } else {
            candidate.exists()
        };
        if exists {
            if let Some(name) = extractor(&candidate) {
                return Some(name);
            }
        }
    }
    None
}

fn extract_dirname(p: &Path) -> Option<String> {
    p.parent()?.file_name()?.to_str().map(String::from)
}

fn extract_package_json(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    let val: serde_json::Value = serde_json::from_str(&text).ok()?;
    val.get("name")?.as_str().map(String::from)
}

fn extract_pyproject(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    let table: toml::Table = text.parse().ok()?;
    if let Some(name) = table
        .get("project")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
    {
        return Some(name.to_string());
    }
    table
        .get("tool")
        .and_then(|v| v.get("poetry"))
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_cargo(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    let table: toml::Table = text.parse().ok()?;
    table
        .get("package")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_gomod(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            let module_path = rest.trim().trim_end_matches('/');
            return module_path
                .split('/')
                .next_back()
                .filter(|s| !s.is_empty())
                .map(String::from);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn mem_conn_with_schema() -> Connection {
        let mut conn = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut conn).unwrap();
        conn
    }

    #[test]
    fn git_marker_returns_dirname() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("my-app");
        std::fs::create_dir_all(app.join(".git")).unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(app.to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "my-app");
    }

    #[test]
    fn package_json_name() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name":"frontend"}"#).unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(dir.path().to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "frontend");
    }

    #[test]
    fn pyproject_project_name() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"mylib\"\n",
        )
        .unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(dir.path().to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "mylib");
    }

    #[test]
    fn pyproject_poetry_name() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.poetry]\nname = \"poem\"\n",
        )
        .unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(dir.path().to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "poem");
    }

    #[test]
    fn cargo_toml_name() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"crateX\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(dir.path().to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "crateX");
    }

    #[test]
    fn go_mod_last_segment() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/user/mymod\n\ngo 1.21\n",
        )
        .unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(dir.path().to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "mymod");
    }

    #[test]
    fn compose_yml_dirname() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("myservice");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("compose.yml"), "version: '3'\n").unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(app.to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "myservice");
    }

    #[test]
    fn fallback_ungrouped() {
        let dir = TempDir::new().unwrap();
        let deep = dir.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(deep.to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "ungrouped");
    }

    #[test]
    fn depth_cap_20() {
        let dir = TempDir::new().unwrap();
        let mut deep = dir.path().to_path_buf();
        for i in 0..25 {
            deep = deep.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&deep).unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(deep.to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "ungrouped");
    }

    #[test]
    fn cache_hit_no_walk() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("cached-app");
        std::fs::create_dir_all(app.join(".git")).unwrap();
        let conn = mem_conn_with_schema();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?1, 'cached-name', ?2, 0)",
            rusqlite::params![app.to_str().unwrap(), now - 3600],
        )
        .unwrap();
        let name = infer_project(app.to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "cached-name");
    }

    #[test]
    fn stale_cache_walks() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("stale-app");
        std::fs::create_dir_all(app.join(".git")).unwrap();
        let conn = mem_conn_with_schema();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?1, 'old-name', ?2, 0)",
            rusqlite::params![app.to_str().unwrap(), now - 90000],
        )
        .unwrap();
        let name = infer_project(app.to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "stale-app");
    }

    #[test]
    fn cache_refresh_preserves_command_count() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("count-app");
        std::fs::create_dir_all(&app).unwrap();
        let conn = mem_conn_with_schema();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?1, 'old-name', ?2, 42)",
            rusqlite::params![app.to_str().unwrap(), now - 90000],
        )
        .unwrap();
        infer_project(app.to_str().unwrap(), &conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT command_count FROM projects WHERE path=?1",
                rusqlite::params![app.to_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 42);
    }

    #[test]
    fn malformed_toml_continues() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "NOT VALID TOML {{{{").unwrap();
        let conn = mem_conn_with_schema();
        let result = infer_project(dir.path().to_str().unwrap(), &conn);
        assert!(result.is_ok());
    }

    #[test]
    fn package_json_priority_over_pyproject() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name":"frontend"}"#).unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"backend\"\n",
        )
        .unwrap();
        let conn = mem_conn_with_schema();
        let name = infer_project(dir.path().to_str().unwrap(), &conn).unwrap();
        assert_eq!(name, "frontend");
    }
}
