// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
//! Preserve the existing UTF-8, sorted-key, Python JSON evidence identity.
//! This is a native serializer, including legacy float exponent formatting.
use serde_json::{Number, Value};
pub(crate) fn json(value: &Value) -> String {
    match value {
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(json).collect::<Vec<_>>().join(",")
        ),
        Value::Object(values) => {
            let sorted: std::collections::BTreeMap<_, _> = values.iter().collect();
            format!(
                "{{{}}}",
                sorted
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("JSON key"),
                        json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Number(number) if number.is_f64() => float(number),
        other => serde_json::to_string(other).expect("validated JSON value"),
    }
}
fn float(number: &Number) -> String {
    let raw = number.to_string();
    let (negative, raw) = raw
        .strip_prefix('-')
        .map(|s| (true, s))
        .unwrap_or((false, raw.as_str()));
    let (mantissa, exponent) = raw
        .split_once(['e', 'E'])
        .map(|(m, e)| (m, e.parse::<i32>().expect("finite JSON exponent")))
        .unwrap_or((raw, 0));
    let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits = mantissa.replace('.', "");
    let Some(first) = digits.bytes().position(|b| b != b'0') else {
        return if negative { "-0.0" } else { "0.0" }.into();
    };
    let power = exponent + decimal - 1 - first as i32;
    let digits = digits[first..].trim_end_matches('0');
    let body = if !(-4..16).contains(&power) {
        let mantissa = if digits.len() == 1 {
            digits.to_string()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!(
            "{mantissa}e{}{:02}",
            if power < 0 { '-' } else { '+' },
            power.unsigned_abs()
        )
    } else {
        let position = power + 1;
        if position <= 0 {
            format!("0.{}{digits}", "0".repeat((-position) as usize))
        } else if position as usize >= digits.len() {
            format!("{digits}{}.0", "0".repeat(position as usize - digits.len()))
        } else {
            format!(
                "{}.{}",
                &digits[..position as usize],
                &digits[position as usize..]
            )
        }
    };
    if negative { format!("-{body}") } else { body }
}
