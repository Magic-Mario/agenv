use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_agenv");

fn run(project: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(project)
        .env("AGENV_STORE", store)
        .output()
        .expect("failed to run agenv")
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn temp_base(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agenv-e2e-{}-{}-{}",
        name,
        std::process::id(),
        unique()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_source(base: &Path) -> PathBuf {
    let src = base.join("skill-src");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    git(&src, &["init", "-q"]);
    git(&src, &["config", "user.email", "t@example.com"]);
    git(&src, &["config", "user.name", "t"]);
    std::fs::write(src.join("SKILL.md"), "# skill\n").unwrap();
    std::fs::write(src.join("sub/a.txt"), "hello\n").unwrap();
    git(&src, &["add", "."]);
    git(&src, &["commit", "-qm", "init"]);
    git(&src, &["tag", "v1.0.0"]);
    src
}

fn make_monorepo(base: &Path) -> PathBuf {
    let src = base.join("monorepo");
    std::fs::create_dir_all(src.join("skills/engineering/grill-with-docs")).unwrap();
    std::fs::create_dir_all(src.join("skills/other")).unwrap();
    git(&src, &["init", "-q"]);
    git(&src, &["config", "user.email", "t@example.com"]);
    git(&src, &["config", "user.name", "t"]);
    std::fs::write(
        src.join("skills/engineering/grill-with-docs/SKILL.md"),
        "# grill\n",
    )
    .unwrap();
    std::fs::write(src.join("skills/other/SKILL.md"), "# other\n").unwrap();
    std::fs::write(src.join("README.md"), "# monorepo\n").unwrap();
    git(&src, &["add", "."]);
    git(&src, &["commit", "-qm", "init"]);
    git(&src, &["tag", "v1.0.0"]);
    src
}

fn find_file(root: &Path, name: &str) -> PathBuf {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().unwrap() == name {
                return p;
            }
        }
    }
    panic!("{name} not found under {}", root.display());
}

#[test]
fn full_flow() {
    let base = temp_base("flow");
    let src = make_source(&base);
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    let source = src.to_string_lossy().into_owned();

    let out = run(&project, &store, &["init"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(project.join("agenv.toml").exists());

    let out = run(
        &project,
        &store,
        &["add", "foo", "--source", &source, "--ref", "v1.0.0"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run(&project, &store, &["install"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(project.join(".claude/skills/foo/SKILL.md").exists());
    assert!(project.join("agenv.lock").exists());
    let installed = std::fs::read(project.join(".claude/skills/foo/SKILL.md")).unwrap();

    let out = run(&project, &store, &["status"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // mutate installed content -> drift
    std::fs::write(project.join(".claude/skills/foo/SKILL.md"), "# tampered\n").unwrap();
    let out = run(&project, &store, &["status"]);
    assert!(!out.status.success());

    // reinstall reproduces the locked content
    let out = run(&project, &store, &["install"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read(project.join(".claude/skills/foo/SKILL.md")).unwrap(),
        installed
    );
    let out = run(&project, &store, &["status"]);
    assert!(out.status.success());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn deterministic_lock() {
    let base = temp_base("deterministic");
    let src = make_source(&base);
    let store = base.join("store");
    let source = src.to_string_lossy().into_owned();

    let mut locks = Vec::new();
    for i in 0..2 {
        let project = base.join(format!("project{i}"));
        std::fs::create_dir_all(&project).unwrap();
        assert!(run(&project, &store, &["init"]).status.success());
        assert!(run(
            &project,
            &store,
            &["add", "foo", "--source", &source, "--ref", "v1.0.0"]
        )
        .status
        .success());
        assert!(run(&project, &store, &["install"]).status.success());
        locks.push(std::fs::read(project.join("agenv.lock")).unwrap());
    }
    assert_eq!(locks[0], locks[1]);

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn update_moves_to_new_commit() {
    let base = temp_base("update");
    let src = make_source(&base);
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    let source = src.to_string_lossy().into_owned();

    assert!(run(&project, &store, &["init"]).status.success());
    assert!(run(
        &project,
        &store,
        &["add", "foo", "--source", &source, "--ref", "v1.0.0"]
    )
    .status
    .success());
    assert!(run(&project, &store, &["install"]).status.success());
    let old_commit = read_lock_commit(&project.join("agenv.lock"));

    // move the tag to a new commit
    std::fs::write(src.join("sub/b.txt"), "new\n").unwrap();
    git(&src, &["add", "."]);
    git(&src, &["commit", "-qm", "second"]);
    git(&src, &["tag", "-f", "v1.0.0"]);

    let out = run(&project, &store, &["update", "foo"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let new_commit = read_lock_commit(&project.join("agenv.lock"));
    assert_ne!(old_commit, new_commit);

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn checksum_mismatch_aborts() {
    let base = temp_base("corrupt");
    let src = make_source(&base);
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    let source = src.to_string_lossy().into_owned();

    assert!(run(&project, &store, &["init"]).status.success());
    assert!(run(
        &project,
        &store,
        &["add", "foo", "--source", &source, "--ref", "v1.0.0"]
    )
    .status
    .success());
    assert!(run(&project, &store, &["install"]).status.success());
    let before = std::fs::read_to_string(project.join(".claude/skills/foo/SKILL.md")).unwrap();

    // corrupt the store tree, not the materialized output
    let stored = find_file(&store, "SKILL.md");
    std::fs::write(&stored, "# corrupted\n").unwrap();

    let out = run(&project, &store, &["install"]);
    assert!(!out.status.success());
    let after = std::fs::read_to_string(project.join(".claude/skills/foo/SKILL.md")).unwrap();
    assert_eq!(before, after, "prior install must remain untouched");

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn subdirectory_skill() {
    let base = temp_base("subdir");
    let src = make_monorepo(&base);
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    let source = src.to_string_lossy().into_owned();

    assert!(run(&project, &store, &["init"]).status.success());

    let out = run(
        &project,
        &store,
        &[
            "add",
            "--source",
            &source,
            "--ref",
            "v1.0.0",
            "--path",
            "skills/engineering/grill-with-docs",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // name inferred from the path
    let manifest = std::fs::read_to_string(project.join("agenv.toml")).unwrap();
    assert!(manifest.contains("grill-with-docs"), "{manifest}");

    let out = run(&project, &store, &["install"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let materialized =
        std::fs::read_to_string(project.join(".claude/skills/grill-with-docs/SKILL.md")).unwrap();
    assert!(materialized.contains("grill"), "{materialized}");
    // only the requested subdirectory is materialized, not the whole repo
    assert!(!project.join(".claude/skills/other").exists());
    assert!(!project
        .join(".claude/skills/grill-with-docs/README.md")
        .exists());

    let out = run(&project, &store, &["status"]);
    assert!(out.status.success());

    let _ = std::fs::remove_dir_all(&base);
}

fn read_lock_commit(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let doc: toml::Value = toml::from_str(&text).unwrap();
    doc["skills"][0]["commit"].as_str().unwrap().to_string()
}

fn edit_manifest(project: &Path, f: impl FnOnce(&mut toml::Value)) {
    let path = project.join("agenv.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut doc: toml::Value = toml::from_str(&text).unwrap();
    f(&mut doc);
    std::fs::write(&path, toml::to_string(&doc).unwrap()).unwrap();
}

#[test]
fn sync_multi_harness_prunes() {
    let base = temp_base("sync");
    let src = make_source(&base);
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    let source = src.to_string_lossy().into_owned();

    assert!(run(&project, &store, &["init"]).status.success());

    edit_manifest(&project, |m| {
        m["harness"] = toml::Value::Array(vec![
            toml::Value::String("claude-code".into()),
            toml::Value::String("opencode".into()),
        ]);
    });

    assert!(run(
        &project,
        &store,
        &["add", "foo", "--source", &source, "--ref", "v1.0.0"]
    )
    .status
    .success());

    let out = run(&project, &store, &["sync"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(project.join(".claude/skills/foo/SKILL.md").exists());
    assert!(project.join(".opencode/skills/foo/SKILL.md").exists());

    edit_manifest(&project, |m| {
        m["skills"].as_table_mut().unwrap().remove("foo");
    });

    let out = run(&project, &store, &["sync", "--prune"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!project.join(".claude/skills/foo").exists());
    assert!(!project.join(".opencode/skills/foo").exists());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn local_skill_install_no_lock_entry() {
    let base = temp_base("local-install");
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(project.join("skills/my-skill")).unwrap();
    std::fs::write(project.join("skills/my-skill/SKILL.md"), "# mine\n").unwrap();

    assert!(run(&project, &store, &["init"]).status.success());
    assert!(run(
        &project,
        &store,
        &["add", "my-skill", "--source", "./skills/my-skill"]
    )
    .status
    .success());
    assert!(run(&project, &store, &["install"]).status.success());

    assert!(project.join(".claude/skills/my-skill/SKILL.md").exists());
    let lock = std::fs::read_to_string(project.join("agenv.lock")).unwrap();
    assert!(!lock.contains("my-skill"), "{lock}");

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn sync_adopts_harness_skill_vendors() {
    let base = temp_base("adopt-harness");
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(project.join(".claude/skills/stray")).unwrap();
    std::fs::write(project.join(".claude/skills/stray/SKILL.md"), "# stray\n").unwrap();

    assert!(run(&project, &store, &["init"]).status.success());

    let out = run(&project, &store, &["sync"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = std::fs::read_to_string(project.join("agenv.toml")).unwrap();
    assert!(manifest.contains("[skills.stray]"), "{manifest}");
    assert!(
        manifest.contains("source = \"./skills/stray\""),
        "{manifest}"
    );
    assert!(project.join("skills/stray/SKILL.md").exists());
    assert!(project.join(".claude/skills/stray/SKILL.md").exists());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn sync_adopts_local_skills_dir() {
    let base = temp_base("adopt-local");
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(project.join("skills/mine")).unwrap();
    std::fs::write(project.join("skills/mine/SKILL.md"), "# mine\n").unwrap();

    assert!(run(&project, &store, &["init"]).status.success());

    let out = run(&project, &store, &["sync"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = std::fs::read_to_string(project.join("agenv.toml")).unwrap();
    assert!(
        manifest.contains("source = \"./skills/mine\""),
        "{manifest}"
    );
    assert!(project.join(".claude/skills/mine/SKILL.md").exists());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn sync_prune_deletes_undeclared() {
    let base = temp_base("prune");
    let project = base.join("project");
    let store = base.join("store");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(project.join(".claude/skills/extra")).unwrap();
    std::fs::write(project.join(".claude/skills/extra/SKILL.md"), "# extra\n").unwrap();

    assert!(run(&project, &store, &["init"]).status.success());

    let out = run(&project, &store, &["sync", "--prune"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(!project.join(".claude/skills/extra").exists());
    let manifest = std::fs::read_to_string(project.join("agenv.toml")).unwrap();
    assert!(!manifest.contains("extra"), "{manifest}");

    let _ = std::fs::remove_dir_all(&base);
}
