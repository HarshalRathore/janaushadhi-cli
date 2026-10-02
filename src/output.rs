//! Output rendering: table / TSV / JSON / CSV with the agreed display rules.
//!
//! - default on a TTY: table; when piped: plain TSV
//! - `--json` / `--csv` for machines — never silently truncated
//! - data goes to stdout, diagnostics to stderr
//! - broken pipes (e.g. `| head`) exit quietly

use std::io::{self, IsTerminal, Write};

use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use serde_json::Value;

use crate::api::{ApiError, AppErr};
use crate::registry::Col;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Table,
    Tsv,
    Json,
    Csv,
}

pub fn mode_from(json: bool, csv: bool, table: bool) -> Mode {
    if json {
        Mode::Json
    } else if csv {
        Mode::Csv
    } else if table {
        Mode::Table
    } else if io::stdout().is_terminal() {
        Mode::Table
    } else {
        Mode::Tsv
    }
}

pub fn cell_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            // Render whole floats as integers (45.0 -> 45) for friendlier tables.
            if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    return format!("{}", f as i64);
                }
            }
            n.to_string()
        }
        _ => serde_json::to_string(v).unwrap_or_default(),
    }
}

fn field(rec: &Value, key: &str) -> String {
    rec.get(key).map(cell_string).unwrap_or_default()
}

fn sanitize(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn new_table(headers: &[&str]) -> Table {
    let mut t = Table::new();
    t.load_preset(UTF8_FULL);
    t.set_content_arrangement(ContentArrangement::Dynamic);
    t.set_header(headers);
    t
}

pub fn render_table(cols: &[Col], records: &[Value]) {
    let headers: Vec<&str> = cols.iter().map(|c| c.header).collect();
    let mut t = new_table(&headers);
    for r in records {
        t.add_row(cols.iter().map(|c| field(r, c.key)));
    }
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{t}");
}

pub fn render_tsv(cols: &[Col], records: &[Value]) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let header = cols.iter().map(|c| sanitize(c.header)).collect::<Vec<_>>().join("\t");
    let _ = writeln!(lock, "{header}");
    for r in records {
        let line = cols.iter().map(|c| sanitize(&field(r, c.key))).collect::<Vec<_>>().join("\t");
        let _ = writeln!(lock, "{line}");
    }
}

pub fn render_csv(cols: &[Col], records: &[Value]) -> Result<(), AppErr> {
    let mut wtr = csv::Writer::from_writer(io::stdout());
    let header: Vec<&str> = cols.iter().map(|c| c.header).collect();
    wtr.write_record(header.iter()).map_err(map_csv)?;
    for r in records {
        let row: Vec<String> = cols.iter().map(|c| field(r, c.key)).collect();
        wtr.write_record(row.iter()).map_err(map_csv)?;
    }
    wtr.flush().map_err(map_io)?;
    Ok(())
}

fn map_csv(e: csv::Error) -> AppErr {
    if let csv::ErrorKind::Io(ioe) = e.kind() {
        if ioe.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
    }
    AppErr::Api(ApiError::Parse(format!("csv write failed: {e}")))
}

fn map_io(e: io::Error) -> AppErr {
    if e.kind() == io::ErrorKind::BrokenPipe {
        std::process::exit(0);
    }
    AppErr::Api(ApiError::Parse(format!("write failed: {e}")))
}

pub fn render_json(dataset: &str, records: &[Value], total: Option<u64>) {
    let out = serde_json::json!({
        "dataset": dataset,
        "total": total,
        "count": records.len(),
        "records": records,
    });
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

pub fn print_value(v: &Value) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

/// Key/value table for `show` commands.
pub fn print_kv(record: &Value) {
    if let Value::Object(map) = record {
        let mut t = new_table(&["Field", "Value"]);
        for (k, v) in map {
            t.add_row([k.clone(), cell_string(v)]);
        }
        let stdout = io::stdout();
        let mut lock = stdout.lock();
        let _ = writeln!(lock, "{t}");
    } else {
        print_value(record);
    }
}

/// Generic table from pre-built rows (used by `datasets` and `doctor`).
pub fn rows_table(headers: &[&str], rows: Vec<Vec<String>>) {
    let mut t = new_table(headers);
    for r in rows {
        t.add_row(r);
    }
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{t}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_strings() {
        assert_eq!(cell_string(&serde_json::json!("a")), "a");
        assert_eq!(cell_string(&serde_json::json!(10.32)), "10.32");
        assert_eq!(cell_string(&serde_json::Value::Null), "");
        assert_eq!(cell_string(&serde_json::json!(true)), "true");
    }
}
