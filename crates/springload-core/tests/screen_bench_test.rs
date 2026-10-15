//! 量一遍筛选在美股量级的股票池上要多久：5449 只 × 750 根日线。
use springload_core::screening::{self, ScreenParams};
use springload_core::config::Settings;
use springload_core::storage::bar::{self, BarRow};
use springload_core::storage::stock::{self, Stock};
use springload_core::storage::Database;
use std::time::Instant;

#[test]
#[ignore = "量性能用，跑得慢"]
fn us_sized_universe_screen_cost() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("springload.us.db")).unwrap();
    let now = "2026-10-15T00:00:00Z".to_string();
    let symbols: Vec<String> = (0..5449).map(|i| format!("SYM{i:04}")).collect();
    let started = Instant::now();
    {
        let mut conn = db.conn().unwrap();
        for (index, code) in symbols.iter().enumerate() {
            stock::upsert(&conn, &Stock {
                code: code.clone(), name: code.clone(), market: "US".into(), board: "common".into(),
                sector: None, listed_at: None, is_st: false, updated_at: now.clone(),
            }).unwrap();
            // 一条温和上行的价格路径，混入回落，让判定有活干。
            let bars: Vec<BarRow> = (0..750).map(|day| {
                let base = 20.0 + day as f64 * 0.06 + ((day + index) % 17) as f64 * 0.05;
                let close = base * (1.0 + (((day + index) % 23) as f64 - 11.0) / 400.0);
                BarRow {
                    code: code.clone(), date: format!("2023-{:02}-{:02}", (day / 28) % 12 + 1, day % 28 + 1),
                    open: close, high: close * 1.01, low: close * 0.99, close,
                    volume: 2_000_000.0, amount: 6e7,
                }
            }).collect();
            bar::upsert_many(&mut conn, &bars).unwrap();
        }
    }
    println!("造数据用了 {} 毫秒", started.elapsed().as_millis());

    let params = ScreenParams::from_settings(&Settings::default());
    let started = Instant::now();
    let report = screening::run_screen(&db, &params).unwrap();
    let elapsed = started.elapsed().as_millis();
    println!(
        "筛选 {} 只，入选 {}，未入选 {}，耗时 {} 毫秒",
        report.universe, report.picked.len(), report.rejected.len(), elapsed
    );
    assert_eq!(report.universe, 5449);
    assert!(elapsed < 60_000, "筛选不应超过一分钟，实际 {elapsed} 毫秒");
}
