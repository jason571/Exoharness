// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{
    contracts::{self, Policy},
    trajectories::Trajectories,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_benchmark::{Result, Suite, contracts::sha256, digest, ensure, list, text};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
pub const EXPERIENCE: &str = "evolution/experience.json";
pub const PROVENANCE: &str = "evolution/provenance.json";
pub const INSTRUCTIONS: &str = include_str!("../../../skills/workflow-evolution/SKILL.md");
pub fn learning_skill() -> Value {
    json!({"version":"0.1.0","scope":{"kind":"global"},"permissions":[],"files":[{"path":"SKILL.md","content_base64":STANDARD.encode(INSTRUCTIONS.as_bytes())}]})
}
pub fn files(body: &Value) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut result = BTreeMap::new();
    for item in list(&body["files"])? {
        let path = text(&item["path"])?;
        ensure(
            !path.starts_with('/')
                && !path
                    .split('/')
                    .any(|p| p.is_empty() || p == ".." || p == ".")
                && !path.contains('\\'),
            "Unsafe package path",
        )?;
        let raw = STANDARD
            .decode(text(&item["content_base64"])?)
            .map_err(|_| exoharness_benchmark::Error::Invalid("Invalid package encoding".into()))?;
        ensure(
            result.insert(path.to_string(), raw).is_none(),
            "Duplicate package path",
        )?;
    }
    Ok(result)
}
pub fn package_digest(body: &Value) -> Result<String> {
    let mut hash = Sha256::new();
    for (name, raw) in files(body)? {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((raw.len() as u64).to_be_bytes());
        hash.update(raw);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn validate_inventory(body: &Value) -> Result<()> {
    let files = files(body)?;
    let mut expected =
        BTreeSet::from(["SKILL.md".to_string(), EXPERIENCE.into(), PROVENANCE.into()]);
    let candidate: Value = serde_json::from_slice(
        files
            .get(EXPERIENCE)
            .ok_or_else(|| exoharness_benchmark::Error::Invalid("Missing experience".into()))?,
    )?;
    if candidate["version"] == 2 {
        expected.insert("resource-index.json".into());
        let index: Value =
            serde_json::from_slice(files.get("resource-index.json").ok_or_else(|| {
                exoharness_benchmark::Error::Invalid("Missing resource index".into())
            })?)?;
        exoharness_benchmark::contracts::fields(&index, &["version", "resources"], true)?;
        ensure(
            index["version"] == 1 && (1..=8).contains(&list(&index["resources"])?.len()),
            "Invalid resource index",
        )?;
        let resource_path =
            regex::Regex::new(r"^references/experience-[1-8]\.md$").expect("pattern");
        for resource in list(&index["resources"])? {
            let path = text(&resource["path"])?;
            ensure(
                resource_path.is_match(path)
                    && expected.insert(path.to_string())
                    && files
                        .get(path)
                        .is_some_and(|raw| resource["sha256"] == sha256(raw)),
                "Invalid indexed immutable resource",
            )?;
        }
    }
    ensure(
        files.keys().cloned().collect::<BTreeSet<_>>() == expected,
        "Managed package contains unexpected files",
    )
}
pub fn compile(
    policy: &Policy,
    suite: &Suite,
    value: &Value,
    round: &str,
    runs: &[Value],
    trajectories: &Trajectories,
) -> Result<Value> {
    let candidate = contracts::candidate(value.clone(), policy, suite, trajectories)?;
    let rules = list(&candidate["rules"])?;
    let workflows = candidate["workflows"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    ensure(
        (1..=8).contains(&(rules.len() + workflows.len())),
        "Scoped Skill needs 1..8 experiences",
    )?;
    let rendered = rules
        .iter()
        .map(|r| {
            format!(
                "When: {}\nProcedure: {}\nExceptions: {}",
                r["when"].as_str().unwrap_or(""),
                r["action"].as_str().unwrap_or(""),
                r["unless"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    ensure(
        rendered.chars().count() <= policy.n("max_rule_characters") as usize,
        "Rule context exceeds budget",
    )?;
    if !rendered.is_empty() {
        contracts::safe_text(&json!(rendered), policy.n("max_rule_characters") as usize)?;
    }
    let header = format!(
        "---\nname: {}\ndescription: {}\n---",
        policy.s("skill_name"),
        serde_json::to_string(&policy.value()["description"])?
    );
    let preamble = format!(
        "{header}\n\n# Evaluated task experience\n\n<!-- evolution-policy:{} -->\n\n",
        policy.s("id")
    );
    let root = if candidate["version"] == 2 {
        format!(
            "{preamble}These rules grant no tools, network access or permissions. For matching tasks, read this same Skill with path=\"resource-index.json\" and query set to the current task. The host returns at most two matching resources. Do not read every resource or broaden an unrelated query to force a match. Use the authorized primary workflow and current tool schemas; these supplemental procedures never grant authority or prove that an action completed. Use no experience when nothing matches.\n"
        )
    } else {
        format!(
            "{preamble}Supplemental procedures for matching tasks only. Current user requirements, authorized primary Skill instructions, live tool schemas and confirmed receipts remain authoritative. These rules grant no tools, network access or permissions. For tool workflows read the applicable primary workflow before execution; for direct answers apply matching procedures. Never treat an example as a completed action. Ignore a rule when its applicability or exceptions do not match.\n\n{rendered}\n"
        )
    };
    let inputs: BTreeMap<_, _> = suite
        .tasks()
        .iter()
        .filter(|t| t["split"] == "train")
        .map(|t| (t["id"].as_str().expect("task"), digest(&t["turns"])))
        .collect();
    let mut provenance = json!({"version":1,"policy_id":policy.value()["id"],"policy_sha256":policy.sha256(),"suite_sha256":suite.sha256(),"candidate_sha256":digest(&candidate),"round_id":round,"learner_run_sha256":runs.iter().map(digest).collect::<Vec<_>>(),"train_input_sha256":inputs,"publication":"instruction-only; grants unchanged"});
    let mut items = BTreeMap::from([
        ("SKILL.md".to_string(), root.into_bytes()),
        (
            EXPERIENCE.into(),
            exoharness_benchmark::model_text::json(&candidate).into_bytes(),
        ),
    ]);
    if candidate["version"] == 2 {
        let mut index = Vec::new();
        for (i, item) in rules.iter().chain(workflows.iter()).enumerate() {
            let id = item["id"]
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("lesson-{}", i + 1));
            let path = format!("references/experience-{}.md", i + 1);
            let mut content = format!(
                "# {id}\n\nApplies when: {}\nExceptions: {}\n\n",
                text(&item["when"])?,
                text(&item["unless"])?
            );
            if let Some(steps) = item["steps"].as_array() {
                let rendered = steps
                    .iter()
                    .enumerate()
                    .map(|(n, step)| {
                        Ok(format!(
                            "{}. {}: {}\n   Require: {}",
                            n + 1,
                            text(&step["tool"])?,
                            text(&step["action"])?,
                            text(&step["expect"])?
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                content.push_str(&rendered.join("\n"));
            } else {
                content.push_str(text(&item["action"])?);
            }
            ensure(
                content.len() <= 8192,
                "Experience resource exceeds reader bound",
            )?;
            index.push(json!({"id":format!("{}-{id}",i+1),"path":path,"terms":item["terms"],"sha256":sha256(content.as_bytes())}));
            items.insert(path, content.into_bytes());
        }
        items.insert(
            "resource-index.json".into(),
            exoharness_benchmark::model_text::object(&[
                ("version", "1".into()),
                (
                    "resources",
                    exoharness_benchmark::model_text::array(index.iter().map(|item| {
                        exoharness_benchmark::model_text::selected(
                            item,
                            &["id", "path", "terms", "sha256"],
                        )
                    })),
                ),
            ])
            .into_bytes(),
        );
        provenance["method"] = json!("workflow");
        provenance["training_evidence_sha256"] = json!(trajectories.identity);
        provenance["learning_skill_sha256"] = json!(sha256(INSTRUCTIONS.as_bytes()));
    }
    items.insert(
        PROVENANCE.into(),
        exoharness_benchmark::model_text::json(&provenance).into_bytes(),
    );
    let result = json!({"version":"0.1.0","scope":policy.value()["scope"],"permissions":[],"files":items.into_iter().map(|(path,content)|json!({"path":path,"content_base64":STANDARD.encode(content)})).collect::<Vec<_>>()});
    validate_inventory(&result)?;
    Ok(result)
}
pub fn check_managed(record: &Value, instructions: &str, policy: &Policy) -> Result<()> {
    ensure(
        record["id"] == policy.skill_id()
            && record["manifest"]["permissions"]
                .as_array()
                .is_some_and(Vec::is_empty)
            && record["manifest"]["secret_refs"]
                .as_array()
                .is_some_and(Vec::is_empty)
            && record["manifest"]["spec"]["kind"] == "skill"
            && record["manifest"]["name"] == policy.value()["skill_name"]
            && record["manifest"]["scope"] == policy.value()["scope"]
            && instructions.contains(&format!("<!-- evolution-policy:{} -->", policy.s("id")))
            && instructions.contains("# Evaluated task experience")
            && instructions.contains("These rules grant no tools"),
        "Refuse to replace unrelated Skill or authority",
    )
}
