//! #22 的两个命令：CSV 导入 K 线（前复权日线）与首次启动确认。
//!
//! 字段校验在边界完成：文件级问题（读不到、编码不对、表头缺列）直接返回错误；
//! 行级问题（列数不对、不是数字、日期格式不对）跳过该行并逐条记进 `errors`，
//! 合法行照常写库。

use crate::state::AppState;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use springload_core::config::{Settings, SettingsPatch};
use springload_core::storage::bar::{self, BarRow};
use std::collections::HashMap;
use tauri::State;

/// CSV 必须有的八列，顺序不限。
const REQUIRED_COLUMNS: [&str; 8] = [
    "code", "date", "open", "high", "low", "close", "volume", "amount",
];
/// 需要按数字解析的六列，顺序与 [`BarRow`] 的字段顺序一致。
const NUMERIC_COLUMNS: [&str; 6] = ["open", "high", "low", "close", "volume", "amount"];

/// `import_bars_csv` 的返回：写入行数、跳过行数与逐条错误。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// 写进 `bar` 表的行数。
    pub rows: usize,
    /// 因校验不通过被跳过的行数。
    pub skipped: usize,
    /// 每条错误的可展示文案，带行号与列名。
    pub errors: Vec<String>,
}

/// 把 CSV 文件里的日线增量写进 `bar` 表（同 `(code, date)` 覆盖）。
///
/// 文件有多大由用户决定，解析与写入都可能跑上几秒，挪到线程池里做。
#[tauri::command]
pub async fn import_bars_csv(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportReport, String> {
    let db = state.db();
    tauri::async_runtime::spawn_blocking(move || {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("读取文件 {path} 失败：{e}"))?;
        let parsed = parse_bars_csv(&text)?;
        let mut conn = db.conn().map_err(|e| e.to_string())?;
        let rows = bar::upsert_many(&mut conn, &parsed.bars).map_err(|e| e.to_string())?;
        Ok(ImportReport {
            rows,
            skipped: parsed.skipped,
            errors: parsed.errors,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 记下用户已读过首次启动的风险提示，返回改完的配置。
#[tauri::command]
pub fn confirm_first_run(state: State<'_, AppState>) -> Result<Settings, String> {
    let patch = SettingsPatch {
        first_run_confirmed: Some(true),
        ..Default::default()
    };
    state.settings_update(&patch).map_err(|e| e.to_string())
}

/// 解析结果：可入库的行、被跳过的行数与逐条错误。
struct ParsedCsv {
    bars: Vec<BarRow>,
    skipped: usize,
    errors: Vec<String>,
}

/// 解析 CSV 文本。表头缺列这类文件级问题返回 `Err`；行级问题进 `errors`。
fn parse_bars_csv(text: &str) -> Result<ParsedCsv, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| format!("读取 CSV 表头失败：{e}"))?
        .clone();
    if headers.is_empty() {
        return Err("CSV 文件为空，缺少表头行".to_string());
    }

    let mut index: HashMap<String, usize> = HashMap::new();
    for (position, name) in headers.iter().enumerate() {
        index.entry(name.trim().to_string()).or_insert(position);
    }
    let missing: Vec<&str> = REQUIRED_COLUMNS
        .iter()
        .copied()
        .filter(|column| !index.contains_key(*column))
        .collect();
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|column| format!("「{column}」"))
            .collect::<Vec<_>>()
            .join("、");
        return Err(format!(
            "CSV 表头缺少必需列 {list}，必需列为 code,date,open,high,low,close,volume,amount"
        ));
    }

    let mut bars = Vec::new();
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for (offset, record) in reader.records().enumerate() {
        // 表头占第 1 行，数据行从第 2 行起。
        let line = offset + 2;
        let record = match record {
            Ok(record) => record,
            Err(error) => {
                skipped += 1;
                errors.push(format!("第 {line} 行无法解析：{error}"));
                continue;
            }
        };

        let mut row_errors = Vec::new();

        let code = cell(&record, &index, "code");
        if code.is_empty() {
            row_errors.push(format!("第 {line} 行第 {} 列「code」为空", column_of(&index, "code")));
        }

        let date_raw = cell(&record, &index, "date");
        let date = parse_date(date_raw);
        if date.is_none() {
            row_errors.push(format!(
                "第 {line} 行第 {} 列「date」不是合法日期（应为 YYYY-MM-DD）：{date_raw}",
                column_of(&index, "date")
            ));
        }

        let mut values = [0f64; NUMERIC_COLUMNS.len()];
        for (slot, name) in NUMERIC_COLUMNS.iter().enumerate() {
            let raw = cell(&record, &index, name);
            let column = column_of(&index, name);
            if raw.is_empty() {
                row_errors.push(format!("第 {line} 行第 {column} 列「{name}」为空"));
            } else if let Some(value) = parse_number(raw) {
                values[slot] = value;
            } else {
                row_errors.push(format!(
                    "第 {line} 行第 {column} 列「{name}」不是数字：{raw}"
                ));
            }
        }

        if !row_errors.is_empty() {
            skipped += 1;
            errors.extend(row_errors);
            continue;
        }

        bars.push(BarRow {
            code: code.to_string(),
            date: date.expect("日期校验通过后才到这里"),
            open: values[0],
            high: values[1],
            low: values[2],
            close: values[3],
            volume: values[4],
            amount: values[5],
        });
    }

    Ok(ParsedCsv {
        bars,
        skipped,
        errors,
    })
}

/// 取一行里某一列的原始值（去掉首尾空白）。表头校验已保证列存在。
fn cell<'a>(record: &'a csv::StringRecord, index: &HashMap<String, usize>, name: &str) -> &'a str {
    let position = *index.get(name).expect("表头校验已保证必需列存在");
    record.get(position).unwrap_or("").trim()
}

/// 列在文件里的 1 起始序号，用于错误文案里的「第几列」。
fn column_of(index: &HashMap<String, usize>, name: &str) -> usize {
    index.get(name).expect("表头校验已保证必需列存在") + 1
}

/// 解析 `YYYY-MM-DD`；要求零填充的严格格式。
fn parse_date(raw: &str) -> Option<String> {
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
    if date.format("%Y-%m-%d").to_string() == raw {
        Some(raw.to_string())
    } else {
        None
    }
}

/// 解析一个有限浮点数；`inf`、`NaN` 都不算合法。
fn parse_number(raw: &str) -> Option<f64> {
    let value: f64 = raw.parse().ok()?;
    value.is_finite().then_some(value)
}
