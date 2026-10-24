// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{Result, contracts::sha256, digest, ensure, evidence};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone)]
pub struct Dataset {
    pub root: PathBuf,
    pub groups: PathBuf,
    pub revision: String,
}
impl Dataset {
    pub fn new(root: &Path, revision: &str) -> Result<Self> {
        evidence::no_links(root)?;
        let root = root.canonicalize()?;
        let actual = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["rev-parse", "HEAD"])
            .output()?;
        ensure(
            actual.status.success() && String::from_utf8_lossy(&actual.stdout).trim() == revision,
            "Pin exact upstream GDPevo commit",
        )?;
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["status", "--porcelain", "--untracked-files=all"])
            .output()?;
        ensure(
            status.status.success() && status.stdout.is_empty(),
            "Dataset checkout must be clean",
        )?;
        Self::base(root, revision)
    }
    fn base(root: PathBuf, revision: &str) -> Result<Self> {
        ensure(
            revision.len() == 40
                && revision
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Invalid upstream revision",
        )?;
        Ok(Self {
            groups: root.join("data/task_groups"),
            root,
            revision: revision.to_string(),
        })
    }
    pub fn from_snapshot(root: &Path, revision: &str, manifest: &Value) -> Result<Self> {
        evidence::no_links(root)?;
        let value = Self::base(root.canonicalize()?, revision)?;
        ensure(
            manifest["upstream_revision"] == revision && value.inventory()? == *manifest,
            "Transported snapshot differs from frozen inventory",
        )?;
        Ok(value)
    }
    pub fn path(&self, group: &str, split: &str, task: &str) -> Result<PathBuf> {
        ensure(
            regex::Regex::new(r"^task_group_[0-9]{3}$")
                .expect("pattern")
                .is_match(group)
                && ["train", "test"].contains(&split)
                && task.len() == 3
                && task.bytes().all(|b| b.is_ascii_digit()),
            "Invalid official task identity",
        )?;
        let path = self
            .groups
            .join(group)
            .join(format!("{split}_tasks"))
            .join(task);
        evidence::no_links(&path)?;
        ensure(path.is_dir(), "Official task does not exist")?;
        Ok(path)
    }
    pub fn read(&self, path: &Path) -> Result<Vec<u8>> {
        ensure(path.starts_with(&self.root), "Dataset path outside root")?;
        evidence::bytes(path, 8 * 1024 * 1024)
    }
    pub fn task_ids(&self, group: &str, split: &str) -> Result<Vec<String>> {
        self.path(group, split, "001")?;
        let mut ids = Vec::new();
        for item in std::fs::read_dir(self.groups.join(group).join(format!("{split}_tasks")))? {
            let item = item?;
            evidence::no_links(&item.path())?;
            if item.file_type()?.is_dir() {
                let id = item.file_name().to_string_lossy().to_string();
                self.path(group, split, &id)?;
                ids.push(id);
            }
        }
        ids.sort();
        Ok(ids)
    }
    fn members(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in walkdir::WalkDir::new(root) {
            let entry =
                entry.map_err(|_| crate::Error::Invalid("Dataset traversal failed".into()))?;
            ensure(
                !entry.file_type().is_symlink(),
                "Dataset links are forbidden",
            )?;
            if entry.file_type().is_file() {
                paths.push(entry.path().to_owned());
            }
        }
        paths.sort();
        Ok(paths)
    }
    pub fn environment(&self, group: &str) -> Result<Value> {
        self.path(group, "test", "001")?;
        let base = self.groups.join(group);
        let env = base.join("env");
        let mut files = BTreeMap::new();
        let mut shared = BTreeMap::new();
        for path in self.members(&env)? {
            files.insert(
                path.strip_prefix(&env)
                    .expect("prefix")
                    .to_string_lossy()
                    .to_string(),
                sha256(&evidence::bytes(&path, 64 * 1024 * 1024)?),
            );
        }
        for path in self.members(&base)? {
            let relative = path.strip_prefix(&base).expect("prefix");
            if !["env", "train_tasks", "test_tasks"].contains(
                &relative
                    .components()
                    .next()
                    .expect("entry")
                    .as_os_str()
                    .to_str()
                    .unwrap_or(""),
            ) {
                shared.insert(
                    relative.to_string_lossy().to_string(),
                    sha256(&evidence::bytes(&path, 64 * 1024 * 1024)?),
                );
            }
        }
        let metadata = String::from_utf8(self.read(&base.join("task_group.yaml"))?)
            .map_err(|_| crate::Error::Invalid("Invalid dataset metadata".into()))?;
        let pattern =
            regex::Regex::new(r"(?m)^  state_mode: (read_only|mutable)\s*$").expect("pattern");
        let mode = pattern
            .captures(&metadata)
            .and_then(|m| m.get(1))
            .map(|m| m.as_str())
            .ok_or_else(|| crate::Error::Invalid("Released state mode missing".into()))?;
        Ok(
            json!({"upstream_revision":self.revision,"group":group,"state_mode":mode,"environment_sha256":digest(&json!({"files":files,"shared":shared})),"files":files,"shared_files":shared,"solver_access":"domain API only; deny /judge and train evaluator endpoints","execution":"separate OCI container; never source setup.sh on the host"}),
        )
    }
    pub fn inventory(&self) -> Result<Value> {
        let mut groups = std::fs::read_dir(&self.groups)?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        groups.retain(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("task_group_"))
        });
        groups.sort();
        ensure(!groups.is_empty(), "Empty official dataset")?;
        let (mut tasks, mut environments) = (Vec::new(), Vec::new());
        for base in groups {
            let group = base.file_name().expect("group").to_string_lossy();
            for split in ["train", "test"] {
                for id in self.task_ids(&group, split)? {
                    let path = self.path(&group, split, &id)?;
                    let mut files = BTreeMap::new();
                    for member in self.members(&path)? {
                        files.insert(
                            member
                                .strip_prefix(&path)
                                .expect("prefix")
                                .to_string_lossy()
                                .to_string(),
                            sha256(&self.read(&member)?),
                        );
                    }
                    ensure(
                        ["input/prompt.txt", "output/answer.json", "eval/eval.sh"]
                            .iter()
                            .all(|n| files.contains_key(*n)),
                        "Incomplete released task",
                    )?;
                    tasks.push(json!({"group":group,"split":split,"task":id,"files":files}));
                }
            }
            environments.push(self.environment(&group)?);
        }
        Ok(
            json!({"version":1,"upstream_revision":self.revision,"dataset_sha256":digest(&json!({"tasks":tasks,"environments":environments})),"tasks":tasks,"environments":environments}),
        )
    }
    pub fn solver_input(
        &self,
        group: &str,
        split: &str,
        task: &str,
        origin: &str,
    ) -> Result<Value> {
        let origin = crate::harness::origin(origin)?;
        let path = self.path(group, split, task)?;
        let input = path.join("input");
        let mut files = BTreeMap::new();
        for p in self.members(&input)? {
            if p.file_name().is_some_and(|n| n == "prompt.txt") {
                continue;
            }
            let value = String::from_utf8(self.read(&p)?)
                .map_err(|_| crate::Error::Invalid("Invalid solver input encoding".into()))?;
            files.insert(
                p.strip_prefix(&input)
                    .expect("input")
                    .to_string_lossy()
                    .to_string(),
                value,
            );
        }
        let prompt = String::from_utf8(self.read(&input.join("prompt.txt"))?)
            .map_err(|_| crate::Error::Invalid("Invalid solver prompt encoding".into()))?;
        Ok(json!({"prompt":prompt,"environment_origin":origin,"input_files":files}))
    }
    pub fn training_examples(&self, group: &str, origin: &str) -> Result<Value> {
        let mut values = Vec::new();
        for task in self.task_ids(group, "train")? {
            let mut value = self.solver_input(group, "train", &task, origin)?;
            value["answer"] = serde_json::from_slice(
                &self.read(&self.path(group, "train", &task)?.join("output/answer.json"))?,
            )?;
            values.push(value);
        }
        Ok(json!(values))
    }
}
