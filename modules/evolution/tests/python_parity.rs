use exoharness_benchmark::{Suite, campaign};
use exoharness_evolution::{contracts::Policy, learning, package, trajectories::Trajectories};
use serde_json::{Value, json};

fn cases() -> Vec<Value> {
    let value: Value = serde_json::from_str(include_str!(
        "../../../scripts/tests/fixtures/rust-evaluation/python-parity.json"
    ))
    .unwrap();
    value["cases"].as_array().unwrap().clone()
}
#[test]
fn original_python_model_prompts_are_preserved_exactly() {
    for case in cases() {
        let suite = Suite::parse(case["suite"].clone()).unwrap();
        let policy = Policy::parse(case["policy"].clone()).unwrap();
        let trajectories = Trajectories {
            rows: case["trajectory_rows"].as_array().unwrap().clone(),
            identity: case["trajectory_identity"].as_str().map(str::to_string),
        };
        assert_eq!(
            campaign::learning_prompt(&suite),
            case["expected_inline_prompt"].as_str().unwrap(),
            "inline {}",
            case["name"]
        );
        if let Some(expected) = case["expected_inline_guidance"].as_str() {
            let experience =
                exoharness_benchmark::Experience::parse(case["candidate"].clone(), &suite).unwrap();
            assert_eq!(
                experience.instructions(policy.s("group")),
                expected,
                "inline guidance {}",
                case["name"]
            );
        }
        assert_eq!(
            learning::prompt(
                &policy,
                &suite,
                &case["incumbent"],
                case["feedback"].as_array().unwrap(),
                &trajectories
            )
            .unwrap(),
            case["expected_evolution_prompt"].as_str().unwrap(),
            "evolution {}",
            case["name"]
        );
    }
}
#[test]
fn original_python_skill_files_and_source_digests_are_preserved_exactly() {
    for case in cases() {
        let suite = Suite::parse(case["suite"].clone()).unwrap();
        let policy = Policy::parse(case["policy"].clone()).unwrap();
        let trajectories = Trajectories {
            rows: case["trajectory_rows"].as_array().unwrap().clone(),
            identity: case["trajectory_identity"].as_str().map(str::to_string),
        };
        let body = package::compile(
            &policy,
            &suite,
            &case["candidate"],
            "round-0",
            &[json!("golden-learner")],
            &trajectories,
        )
        .unwrap();
        assert_eq!(body, case["expected_package"], "package {}", case["name"]);
        assert_eq!(
            package::package_digest(&body).unwrap(),
            case["expected_source_digest"].as_str().unwrap(),
            "source digest {}",
            case["name"]
        );
    }
}
