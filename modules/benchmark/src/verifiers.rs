// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{Result, ensure};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read},
    path::Path,
};
pub fn json_answer(text: &str) -> Result<Value> {
    let text = text.trim();
    let text = if text.starts_with("```") && text.ends_with("```") {
        text.split_once('\n')
            .map(|(_, s)| s[..s.len() - 3].trim())
            .unwrap_or(text)
    } else {
        text
    };
    Ok(serde_json::from_str(text)?)
}
pub fn inspect_artifact(name: &str, raw: &[u8], probe: Option<Value>) -> Result<Value> {
    let extension = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_lowercase()))
        .unwrap_or_default();
    let mut result = json!({"extension":extension,"nonempty":!raw.is_empty()});
    if extension == ".html" {
        let text = String::from_utf8(raw.to_vec())
            .map_err(|_| crate::Error::Invalid("Invalid HTML encoding".into()))?;
        result["valid_html"] = json!(
            text.to_lowercase().contains("<html") || text.to_lowercase().contains("<!doctype html")
        );
        result["text"] = json!(text);
    } else if [".zip", ".pptx"].contains(&extension.as_str()) {
        let mut archive = zip::ZipArchive::new(Cursor::new(raw))
            .map_err(|_| crate::Error::Invalid("Invalid delivery archive".into()))?;
        ensure(archive.len() <= 2000, "Archive member limit exceeded")?;
        let (mut size, mut count, mut texts) = (0u64, 0u64, Vec::new());
        for i in 0..archive.len() {
            let mut item = archive
                .by_index(i)
                .map_err(|_| crate::Error::Invalid("Invalid archive entry".into()))?;
            size = size
                .checked_add(item.size())
                .ok_or_else(|| crate::Error::Invalid("Archive size overflow".into()))?;
            ensure(
                size <= 64 * 1024 * 1024 && item.enclosed_name().is_some(),
                "Unsafe or oversized archive",
            )?;
            let name = item.name().to_owned();
            let selected = if extension == ".pptx" {
                name.starts_with("ppt/slides/slide") && name.ends_with(".xml")
            } else {
                name.to_lowercase().ends_with(".html")
            };
            if selected {
                let mut data = String::new();
                item.read_to_string(&mut data)?;
                count += 1;
                if extension == ".pptx" {
                    texts.push(slide_text(&data)?);
                } else {
                    texts.push(data);
                }
            }
        }
        result[if extension == ".pptx" {
            "slides"
        } else {
            "html_files"
        }] = json!(count);
        result["text"] = json!(texts.join(" "));
    } else if let Some(probe) = probe
        && let Some(p) = probe.as_object()
    {
        result.as_object_mut().expect("object").extend(p.clone());
    }
    Ok(result)
}
fn slide_text(data: &str) -> Result<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(data);
    let mut text = String::new();
    let (mut depth, mut roots) = (0usize, 0usize);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
                ensure(depth <= 1024 && roots == 1, "Invalid slide XML structure")?;
                for attribute in e.attributes() {
                    attribute
                        .map_err(|_| crate::Error::Invalid("Invalid slide XML attribute".into()))?
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|_| {
                            crate::Error::Invalid("Invalid slide XML attribute value".into())
                        })?;
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 0 {
                    roots += 1;
                }
                ensure(roots == 1, "Invalid slide XML root")?;
                for attribute in e.attributes() {
                    attribute
                        .map_err(|_| crate::Error::Invalid("Invalid slide XML attribute".into()))?
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|_| {
                            crate::Error::Invalid("Invalid slide XML attribute value".into())
                        })?;
                }
            }
            Ok(Event::End(_)) => {
                ensure(depth > 0, "Invalid slide XML closing element")?;
                depth -= 1;
            }
            Ok(Event::DocType(_)) => {
                return Err(crate::Error::Invalid(
                    "Slide XML declarations cannot define entities".into(),
                ));
            }
            Ok(Event::Text(e)) => {
                let decoded = e
                    .decode()
                    .map_err(|_| crate::Error::Invalid("Invalid slide XML text".into()))?;
                ensure(
                    depth > 0 || decoded.trim().is_empty(),
                    "Slide XML text outside root",
                )?;
                if depth > 0 {
                    text.push_str(&decoded);
                }
            }
            Ok(Event::CData(e)) => text.push_str(
                &e.decode()
                    .map_err(|_| crate::Error::Invalid("Invalid slide XML text".into()))?,
            ),
            Ok(Event::GeneralRef(e)) => {
                ensure(depth > 0, "Slide XML reference outside root")?;
                if let Some(c) = e.resolve_char_ref().map_err(|_| {
                    crate::Error::Invalid("Invalid slide XML character reference".into())
                })? {
                    text.push(c);
                } else {
                    let name = e
                        .decode()
                        .map_err(|_| crate::Error::Invalid("Invalid XML reference".into()))?;
                    let c = match name.as_ref() {
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "quot" => '"',
                        "apos" => '\'',
                        _ => return Err(crate::Error::Invalid("Unsupported XML entity".into())),
                    };
                    text.push(c);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(crate::Error::Invalid("Invalid slide XML".into())),
            _ => {}
        }
    }
    ensure(depth == 0 && roots == 1, "Incomplete slide XML")?;
    Ok(text)
}
pub fn verify(task: &Value, outcome: &Value) -> Value {
    let mut checks = Vec::new();
    for check in task["checks"].as_array().into_iter().flatten() {
        let ok = match check["kind"].as_str().unwrap_or("") {
            "json_equals" => json_answer(outcome["text"].as_str().unwrap_or(""))
                .ok()
                .is_some_and(|a| {
                    check["fields"]
                        .as_object()
                        .is_some_and(|f| f.iter().all(|(k, v)| a.get(k) == Some(v)))
                }),
            "text_contains" => check["values"].as_array().is_some_and(|v| {
                v.iter().all(|t| {
                    outcome["text"]
                        .as_str()
                        .unwrap_or("")
                        .contains(t.as_str().unwrap_or("\0"))
                })
            }),
            "tool_count" => {
                let key = if check["successful_only"] == true {
                    "successful_tool_names"
                } else {
                    "tool_names"
                };
                let count = outcome[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|n| {
                        n.as_str()
                            .unwrap_or("")
                            .contains(check["name_contains"].as_str().unwrap_or("\0"))
                    })
                    .count() as u64;
                count >= check["min"].as_u64().unwrap_or(0)
                    && count <= check["max"].as_u64().unwrap_or(0)
            }
            "artifact" => outcome["artifacts"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| {
                    item["extension"] == check["extension"]
                        && item["nonempty"] == true
                        && (item["extension"] != ".html" || item["valid_html"] == true)
                        && (item["extension"] != ".zip"
                            || item["html_files"].as_u64().unwrap_or(0) > 0)
                        && check.get("slides").is_none_or(|v| *v == item["slides"])
                        && check["contains"].as_array().into_iter().flatten().all(|v| {
                            item["text"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&v.as_str().unwrap_or("\0").to_lowercase())
                        })
                        && check["min_duration"]
                            .as_f64()
                            .is_none_or(|v| item["duration"].as_f64().is_some_and(|n| n >= v))
                        && check["max_duration"]
                            .as_f64()
                            .is_none_or(|v| item["duration"].as_f64().is_some_and(|n| n <= v))
                        && (check["audio"] != true || item["audio"] == true)
                        && (item["extension"] != ".mp4" || item["video"] == true)
                        && (![".mp3", ".wav"].contains(&item["extension"].as_str().unwrap_or(""))
                            || item["audio"] == true)
                }),
            _ => false,
        };
        checks.push(json!({"kind":check["kind"],"passed":ok}));
    }
    json!({"delivered":outcome["state"]=="succeeded"&&outcome["artifact_errors"].as_array().is_none_or(Vec::is_empty)&&!checks.is_empty()&&checks.iter().all(|v|v["passed"]==true),"checks":checks})
}
