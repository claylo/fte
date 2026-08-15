//! Rendering command results as text or a CLIspec `items` envelope.

use serde::Serialize;

/// Which rendering a command should produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Render {
    /// Human-readable lines.
    Text,
    /// A CLIspec `items` envelope.
    Json,
}

/// Serialize rows into the CLIspec collection envelope.
///
/// The wrapper is required rather than a bare array so the envelope can gain
/// fields without breaking consumers.
#[must_use]
pub fn to_json<T: Serialize>(rows: &[T]) -> String {
    let mut map = serde_json::Map::new();
    map.insert(
        "items".into(),
        serde_json::to_value(rows).unwrap_or(serde_json::Value::Array(Vec::new())),
    );
    serde_json::Value::Object(map).to_string()
}

/// Print rows to stdout in the selected rendering.
pub fn emit_items<T: Serialize>(rows: &[T], render: Render, text: impl Fn(&T) -> String) {
    match render {
        Render::Json => println!("{}", to_json(rows)),
        Render::Text => {
            for row in rows {
                println!("{}", text(row));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Row {
        id: String,
        format: String,
    }

    #[test]
    fn json_wraps_rows_in_an_items_envelope() {
        let rows = vec![Row {
            id: "paper".into(),
            format: "jats".into(),
        }];
        let json = to_json(&rows);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["items"].is_array());
        assert_eq!(parsed["items"][0]["id"], "paper");
    }

    #[test]
    fn an_empty_result_is_still_an_envelope() {
        let rows: Vec<Row> = Vec::new();
        let parsed: serde_json::Value = serde_json::from_str(&to_json(&rows)).unwrap();
        assert!(parsed["items"].is_array());
        assert_eq!(parsed["items"].as_array().unwrap().len(), 0);
    }
}
