// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Result, ToolError};
use serde_json::{Value, json};
/// Network resolution is deliberately disabled: schemas cannot fetch URLs.
pub fn validate(schema: &Value, input: &Value) -> Result<()> {
    if let Some(report) = input_diagnostics(schema, input)? {
        return Err(ToolError::Input(format!(
            "工具参数不符合 Schema；未执行：{report}"
        )));
    }
    Ok(())
}

/// Diagnostics contain schema paths/constraints, never the entire rejected
/// source or binary data. Invalid schemas are NOT model-correctable inputs.
pub fn input_diagnostics(schema: &Value, input: &Value) -> Result<Option<Value>> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|_| ToolError::Input("工具 Schema 无效或包含未解析的外部引用；未执行".into()))?;
    let mut errors = Vec::new();
    for error in validator.iter_errors(input).take(8) {
        collect(schema, &error, &mut errors, 0);
    }
    Ok((!errors.is_empty()).then(|| json!({"errors":errors})))
}

fn collect(
    schema: &Value,
    error: &jsonschema::ValidationError<'_>,
    out: &mut Vec<Value>,
    depth: usize,
) {
    if out.len() >= 12 {
        return;
    }
    use jsonschema::error::ValidationErrorKind;
    if depth < 4
        && let ValidationErrorKind::OneOfNotValid { context }
        | ValidationErrorKind::AnyOf { context } = &error.kind
    {
        for branch in context {
            for error in branch {
                collect(schema, error, out, depth + 1);
            }
        }
        return;
    }
    let path = error.schema_path.to_string();
    let expected = schema
        .pointer(&path)
        .filter(|v| serde_json::to_vec(v).is_ok_and(|v| v.len() <= 2048));
    let mut diagnostic = json!({"instance_path":error.instance_path.to_string(),"schema_path":path,"keyword":path.rsplit('/').next().unwrap_or("validation"),"expected":expected});
    match &error.kind {
        ValidationErrorKind::Required { property } => {
            diagnostic["missing_field"] = property.clone()
        }
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            diagnostic["unexpected_fields"] = json!(unexpected)
        }
        _ => {}
    }
    out.push(diagnostic);
}
