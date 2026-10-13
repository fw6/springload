import { useState, type ReactNode } from "react";
import { useBeginnerMode } from "../copy/BeginnerModeContext";
import { useTerm } from "../copy/GlossaryContext";
import { Modal } from "./Modal";

/**
 * 术语胶囊：点一下弹出这个术语的白话解释。
 * 文案来自 `glossary` 命令，未收录的 key 直接渲染 children。
 * 小白模式关闭时退化成纯文本，不显示虚线胶囊与弹窗。
 */
export function TermHint({ termKey, children }: { termKey: string; children?: ReactNode }) {
  const term = useTerm(termKey);
  const { beginnerMode } = useBeginnerMode();
  const [open, setOpen] = useState(false);

  if (term === undefined) return <>{children}</>;
  if (!beginnerMode) return <>{children ?? term.label}</>;

  return (
    <>
      <button type="button" className="term-chip" onClick={() => setOpen(true)}>
        {children ?? term.label}
      </button>
      {open ? (
        <Modal title={term.label} onClose={() => setOpen(false)}>
          <p>{term.plain}</p>
        </Modal>
      ) : null}
    </>
  );
}
