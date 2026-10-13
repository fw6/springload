import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { confirmFirstRun, getSettings, updateSettings } from "../api/settings";
import { firstRunNotes } from "./riskNotes";

/** 小白模式开关的全局状态。 */
export interface BeginnerModeValue {
  /** 是否开启小白模式。读到配置前按开启处理，解释照常显示。 */
  beginnerMode: boolean;
  /** 改小白模式并立即写回配置。 */
  setBeginnerMode: (value: boolean) => Promise<void>;
  /** 是否已读到配置。 */
  loaded: boolean;
}

const DEFAULT_VALUE: BeginnerModeValue = {
  beginnerMode: true,
  setBeginnerMode: async () => {},
  loaded: false,
};

const BeginnerModeContext = createContext<BeginnerModeValue>(DEFAULT_VALUE);

/** 读小白模式开关。没有 Provider 时按开启处理。 */
export function useBeginnerMode(): BeginnerModeValue {
  return useContext(BeginnerModeContext);
}

/**
 * 风险提示弹窗：逐条列出需求文档第 8 章的五条，底部一个确认按钮。
 * 首次启动确认与设置页的重看入口共用这一个弹窗。
 */
export function RiskNotesDialog({
  confirmLabel,
  onConfirm,
  busy = false,
  error = null,
}: {
  confirmLabel: string;
  onConfirm: () => void;
  busy?: boolean;
  error?: string | null;
}) {
  return (
    <div className="modal-backdrop" role="presentation">
      <div className="modal" role="dialog" aria-modal="true" aria-label="风险提示">
        <h3>风险提示</h3>
        <ol
          style={{
            margin: "0 0 16px",
            paddingLeft: 20,
            lineHeight: 1.75,
            fontSize: 14,
            color: "#334155",
          }}
        >
          {firstRunNotes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ol>
        {error !== null ? <p className="hint">确认失败：{error}</p> : null}
        <button type="button" className="primary modal-close" onClick={onConfirm} disabled={busy}>
          {busy ? "保存中…" : confirmLabel}
        </button>
      </div>
    </div>
  );
}

/** 首次启动时弹出风险提示，确认后写进配置，之后不再弹出。 */
function FirstRunGate() {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((settings) => {
        if (alive && !settings.firstRunConfirmed) setOpen(true);
      })
      .catch(() => {
        // 读不到配置时不打扰使用。
      });
    return () => {
      alive = false;
    };
  }, []);

  const confirm = () => {
    setBusy(true);
    setError(null);
    confirmFirstRun()
      .then(() => setOpen(false))
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setBusy(false));
  };

  if (!open) return null;
  return (
    <RiskNotesDialog
      confirmLabel="我已阅读并理解"
      onConfirm={confirm}
      busy={busy}
      error={error}
    />
  );
}

/**
 * 全应用的小白模式状态。启动时读一次配置，并在首次启动时挂上风险提示弹窗。
 * 这是本 ticket 在 `main.tsx` 里唯一新增的一层接线。
 */
export function BeginnerModeProvider({ children }: { children: ReactNode }) {
  const [beginnerMode, setMode] = useState(true);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((settings) => {
        if (!alive) return;
        setMode(settings.beginnerMode);
        setLoaded(true);
      })
      .catch(() => {
        if (alive) setLoaded(true);
      });
    return () => {
      alive = false;
    };
  }, []);

  const setBeginnerMode = useCallback(async (value: boolean) => {
    const next = await updateSettings({ beginnerMode: value });
    setMode(next.beginnerMode);
  }, []);

  const value = useMemo(
    () => ({ beginnerMode, setBeginnerMode, loaded }),
    [beginnerMode, setBeginnerMode, loaded],
  );

  return (
    <BeginnerModeContext.Provider value={value}>
      {children}
      <FirstRunGate />
    </BeginnerModeContext.Provider>
  );
}
