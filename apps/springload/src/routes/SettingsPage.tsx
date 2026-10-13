import { useEffect, useState } from "react";
import { getAppInfo } from "../api/app";
import { getSettings, updateSettings, type Settings } from "../api/settings";
import { riskNotes } from "../copy/riskNotes";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** 设置页最小可用版本：读写配置里的开关与阈值。本页面后续由 ticket #22 完善。 */
export default function SettingsPage() {
  const [saved, setSaved] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [dataSource, setDataSource] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        setSaved(settings);
        setDraft(settings);
      })
      .catch((err: unknown) => setError(message(err)));
    getAppInfo()
      .then((info) => setDataSource(info.dataSource))
      .catch(() => setDataSource(null));
  }, []);

  if (error !== null && draft === null) {
    return (
      <div className="page">
        <div className="notice" data-tone="risk">设置读取失败：{error}</div>
      </div>
    );
  }

  if (draft === null || saved === null) {
    return (
      <div className="page">
        <p className="hint">正在读取设置…</p>
      </div>
    );
  }

  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);

  const patch = (changes: Partial<Settings>) => setDraft({ ...draft, ...changes });

  const save = async () => {
    setSaving(true);
    try {
      const next = await updateSettings(draft);
      setSaved(next);
      setDraft(next);
      setError(null);
    } catch (err: unknown) {
      setError(message(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 680 }}>
        <div className="panel-head">
          <h2>设置</h2>
          <span className="toolbar-spacer" />
          {dirty ? <span className="hint">有未保存的改动</span> : null}
        </div>
        <div className="panel-body">
          <div className="field-row">
            <div className="field">
              <label>小白模式（术语弹窗解释）</label>
              <select
                value={draft.beginnerMode ? "on" : "off"}
                onChange={(event) => patch({ beginnerMode: event.target.value === "on" })}
              >
                <option value="on">开启</option>
                <option value="off">关闭</option>
              </select>
            </div>
            <div className="field">
              <label>单笔风险比例（%）</label>
              <input
                type="number"
                step="0.1"
                value={draft.riskPct * 100}
                onChange={(event) => patch({ riskPct: Number(event.target.value) / 100 })}
              />
            </div>
            <div className="field">
              <label>止损距离阈值（%）</label>
              <input
                type="number"
                step="0.1"
                value={draft.stopDistanceWarn * 100}
                onChange={(event) => patch({ stopDistanceWarn: Number(event.target.value) / 100 })}
              />
            </div>
            <div className="field">
              <label>登录时自动启动</label>
              <select
                value={draft.autostart ? "on" : "off"}
                onChange={(event) => patch({ autostart: event.target.value === "on" })}
              >
                <option value="on">开启</option>
                <option value="off">关闭</option>
              </select>
            </div>
          </div>

          <div className="field">
            <label>数据源</label>
            <input value={dataSource ?? "未取到"} readOnly />
          </div>

          <div className="notice" data-tone="risk">
            {riskNotes.highReturn}
            <br />
            {riskNotes.market}
            <br />
            {riskNotes.tPlus1}
          </div>

          <div className="panel-foot" style={{ borderTop: "none", padding: 0 }}>
            <button type="button" className="primary" onClick={save} disabled={!dirty || saving}>
              {saving ? "保存中…" : "保存"}
            </button>
            {error !== null ? <span className="hint">保存失败：{error}</span> : null}
          </div>
        </div>
      </div>
    </div>
  );
}
