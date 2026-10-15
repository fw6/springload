import { useCallback, useEffect, useRef, useState } from "react";
import { explain, explainInvalidate, explainRetry } from "../api/ai";
import type { StepIndex } from "../api/types";
import { Button } from "./ui/button";
import { Typesetting } from "./ui/typesetting";

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
 * AI 解释：版面上的一则边栏，用虚线的框圈起来，与正文分开。
 * 自己调命令、管加载与失败状态，失败时显示「解释生成失败」与重试按钮，不沿用上一次的解释。
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
    <section
      data-slot="ai-card"
      aria-label="AI 解释"
      className="flex flex-col gap-2 border border-dashed border-ink bg-slip px-3 py-2.5 text-body leading-relaxed text-ink"
    >
      <span className="flex items-center gap-2 text-micro tracking-[0.08em] text-ink-soft">
        <span>AI 解释</span>
        <span className="border border-rule px-1">AI 生成</span>
      </span>
      {state.kind === "loading" ? <Typesetting>解释生成中…</Typesetting> : null}
      {state.kind === "ready" ? <p>{state.text}</p> : null}
      {state.kind === "error" ? (
        <div className="flex items-center gap-3 text-ink-soft">
          解释生成失败
          <Button size="sm" variant="secondary" onClick={() => void load(code, true)}>
            重试
          </Button>
        </div>
      ) : null}
      <p className="text-micro text-ink-soft">{DISCLAIMER}</p>
    </section>
  );
}
