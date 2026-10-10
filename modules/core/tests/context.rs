use exoharness_core::context::{ContextItem as C, JournalEntry as E, ProjectionError, project};
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn request_replay_is_deterministic_and_append_stable_without_logged_changes() {
    let entries = vec![E::User("go".into()), exchange("inv1")];
    let prefix = project(&entries, &BTreeSet::new()).unwrap();
    let mut extended = entries.clone();
    extended.push(E::Assistant("done".into()));
    let full = project(&extended, &BTreeSet::new()).unwrap();
    assert_eq!(full[..prefix.len()], prefix);
    assert_eq!(full, project(&extended, &BTreeSet::new()).unwrap());
    assert!(matches!(&full[1], C::ToolExchange { .. }));
}

fn exchange(id: &str) -> E {
    E::ToolExchange {
        invocation_id: id.into(),
        tool: "some-tool".into(),
        arguments: json!({"input":"x".repeat(5000)}),
        result: json!({"data":"y".repeat(5000)}),
    }
}

#[test]
fn checkpoint_removes_both_large_arguments_and_results_without_changing_originals() {
    let entries = vec![
        E::User("go".into()),
        exchange("inv1"),
        E::Checkpoint {
            invocation_id: "inv1".into(),
            reference: "store://checkpoints/1".into(),
            summary: json!({"next":"render"}),
        },
    ];
    let original = entries.clone();
    let out = project(&entries, &BTreeSet::new()).unwrap();
    assert_eq!(out.len(), 2);
    assert!(
        matches!(&out[1], C::Checkpoint { summary, .. } if summary == &json!({"next":"render"}))
    );
    assert_eq!(entries, original);
}

#[test]
fn selected_skill_revision_is_loaded_once_and_unselected_skills_are_absent() {
    let skill = E::SkillLoaded {
        revision_key: "writing@2".into(),
        instructions: "instructions".into(),
    };
    let entries = vec![
        skill.clone(),
        E::SkillLoaded {
            revision_key: "unrelated@9".into(),
            instructions: "not selected".into(),
        },
        skill,
    ];
    assert_eq!(
        project(&entries, &BTreeSet::from(["writing@2".into()]))
            .unwrap()
            .len(),
        1
    );
    assert!(project(&entries, &BTreeSet::new()).unwrap().is_empty());
}

#[test]
fn malformed_history_and_conflicting_immutable_revisions_fail_closed() {
    assert!(matches!(
        project(&[exchange("1"), exchange("1")], &BTreeSet::new()),
        Err(ProjectionError::DuplicateInvocation(_))
    ));
    assert!(matches!(
        project(
            &[E::Checkpoint {
                invocation_id: "missing".into(),
                reference: "ref".into(),
                summary: json!({})
            }],
            &BTreeSet::new()
        ),
        Err(ProjectionError::OrphanCheckpoint(_))
    ));
    assert!(matches!(
        project(
            &[
                E::SkillLoaded {
                    revision_key: "s@1".into(),
                    instructions: "a".into()
                },
                E::SkillLoaded {
                    revision_key: "s@1".into(),
                    instructions: "b".into()
                }
            ],
            &BTreeSet::new()
        ),
        Err(ProjectionError::ConflictingSkill(_))
    ));
}
