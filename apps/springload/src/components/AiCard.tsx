import { useCallback, useEffect, useRef, useState } from "react";
import { explain, explainInvalidate, explainRetry } from "../api/ai";
import type { StepIndex } from "../api/types";

/** 免责声明：卡片固定带着，不随模型输出变化。 */
const DISCLAIMER = "以上只是对图上形态与数据的描述，不构成买卖建议。";

export interface AiCardProps {
  /** 解释对象；为 null 时不渲染卡片 */
  code: string | null;
  /** 1~4 对应主流程四步，'journal' 是交易日志页的复盘总结 */
  scope: StepIndex | "journal";
  /** 值变化时让该步的缓存失效并重新生成（第 2 步在增删或拖动标注后传标注指纹） */
  invalidateToken?: string;
}

type CardState =
  | { kind: "loading" }
  | { kind: "ready"; text: string }
  | { kind: "error" };

/**
 * AI 解释卡片：自己调命令、管加载与失败状态，失败时显示「解释生成失败」与重试按钮，
 * 不沿用上一次的解释。固定带「AI 生成」标签与免责声明。
 */
export function AiCard({ code, scope, invalidateToken }: AiCardProps) {
  const [state, setState] = useState<CardState>({ kind: "loading" });
  const requestId = useRef(0);
  const lastToken = useRef(invalidateToken);

  const load = useCallback(
    async (target: string, force: boolean) => {
      const id = ++requestId.current;
      setState({ kind: "loading" });
      try {
        const result = force ? await explainRetry(target, scope) : await explain(target, scope);
        if (id === requestId.current) setState({ kind: "ready", text: result.text });
      } catch {
        if (id === requestId.current) setState({ kind: "error" });
      }
    },
    [scope],
  );

  useEffect(() => {
    if (code === null) return;
    const tokenChanged = lastToken.current !== invalidateToken;
    lastToken.current = invalidateToken;
    if (!tokenChanged) {
      void load(code, false);
      return;
    }
    // 标注变了：先让这一步的缓存失效，再重新生成。
    const id = ++requestId.current;
    setState({ kind: "loading" });
    void (async () => {
      try {
        await explainInvalidate(code, scope);
        const result = await explain(code, scope);
        if (id === requestId.current) setState({ kind: "ready", text: result.text });
      } catch {
        if (id === requestId.current) setState({ kind: "error" });
      }
    })();
  }, [code, scope, invalidateToken, load]);

  if (code === null) return null;

  return (
    <div className="ai-card">
      <span className="ai-tag">AI 生成</span>
      {state.kind === "loading" ? <div>解释生成中…</div> : null}
      {state.kind === "ready" ? <div>{state.text}</div> : null}
      {state.kind === "error" ? (
        <div className="hint">
          解释生成失败
          <button type="button" className="ghost" onClick={() => void load(code, true)}>
            重试
          </button>
        </div>
      ) : null}
      <span className="disclaimer">{DISCLAIMER}</span>
    </div>
  );
}
