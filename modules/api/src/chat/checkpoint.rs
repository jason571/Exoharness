//! Host-owned receipt projection. Called only after exact invocation commit.
use exoharness_agent::DurableReference;
use exoharness_tools::{Binding, Catalog};
use serde_json::{Value, json};

pub(super) fn external(
    catalog: &Catalog,
    binding: &Binding,
    id: &str,
    result: &Value,
) -> Option<DurableReference> {
    if result["isError"] == true || result["is_error"] == true {
        return None;
    }
    let meta = result
        .pointer("/_meta/exoharness")
        .or_else(|| result.pointer("/structuredContent/_meta/exoharness"))?;
    let declared = meta["context_checkpoint"].as_str()?.trim();
    if declared.is_empty() {
        return None;
    }
    let mut summary = json!({"isError":false,"declared_checkpoint":declared});
    // Preserve explicitly named JSON values exactly, never slice authored JSON.
    if let Some(paths) = meta.get("context_preserve") {
        let paths = paths.as_array()?;
        let mut preserved = serde_json::Map::new();
        for path in paths {
            let path = path.as_str()?;
            if !path.starts_with('/') {
                return None;
            }
            let value = result
                .pointer(path)
                .or_else(|| result["structuredContent"].pointer(path))?;
            preserved.insert(path.into(), value.clone());
        }
        summary["preserved_values"] = Value::Object(preserved);
    }
    // Deliverables and typed state are useful even when the service omits a
    // preserve list. Full text content and source arguments remain in journal.
    let data = result.get("structuredContent").unwrap_or(result);
    for key in ["status", "message", "artifacts"] {
        if let Some(value) = data.get(key) {
            summary[key] = value.clone();
        }
    }
    if let Some(directive) = meta.get("workflow_continuation") {
        let names = catalog.resolve_workflow(binding, directive)?;
        if directive["status"] == "complete" {
            // Preserve all structured delivery facts, not only media links.
            // This never cuts a JSON object or invents output metadata.
            let mut delivery = data.clone();
            if let Some(object) = delivery.as_object_mut() {
                object.remove("_meta");
                object.remove("_host_execution");
                if data.get("structuredContent").is_some() {
                    object.remove("content");
                }
            }
            summary["delivery"] = delivery;
        }
        summary["_host_workflow"] = json!({"version":1,"capability":binding.capability,
            "revision":binding.revision,"directive":directive,"tools":names,
            "provider_tools":catalog.provider_tools(binding)});
    } else {
        // An unstructured declaration is not sufficient to discard result
        // content. Require explicit preserve paths for non-workflow receipts.
        meta.get("context_preserve")?;
    }
    Some(DurableReference {
        reference: format!("urn:exoharness:invocation:{id}"),
        summary,
    })
}
