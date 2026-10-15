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
import { Modal } from "../components/Modal";
import { useMarket } from "./MarketContext";
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
 * 首次启动确认与设置页的重看入口共用这一个弹窗；交易制度那一条按当前市场取。
 */
export function RiskNotesDialog({
  open,
  confirmLabel,
  onConfirm,
  busy = false,
  error = null,
}: {
  open: boolean;
  confirmLabel: string;
  onConfirm: () => void;
  busy?: boolean;
  error?: string | null;
}) {
  const { market } = useMarket();

  return (
    <Modal
      open={open}
      title="风险提示"
      onClose={onConfirm}
      confirm={{ label: busy ? "保存中…" : confirmLabel, onClick: onConfirm, disabled: busy }}
    >
      <ol className="list-decimal space-y-2 pl-5">
        {firstRunNotes(market).map((note) => (
          <li key={note}>{note}</li>
        ))}
      </ol>
      {error !== null ? <p className="text-alert">确认失败：{error}</p> : null}
    </Modal>
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
      .catch((cause: unknown) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => setBusy(false));
  };

  return (
    <RiskNotesDialog
      open={open}
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
    <BeginnerModeContext value={value}>
      {children}
      <FirstRunGate />
    </BeginnerModeContext>
  );
}
