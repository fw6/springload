import type { ReactNode } from "react";
import { Button } from "./ui/button";
import { Dialog } from "./ui/dialog";

/**
 * 通用弹窗：面板从正中压印上来，点遮罩、Esc 或底部的确认按钮关闭。
 * 确认按钮默认是「知道了」，需要别的动作（例如风险提示的确认）时用 `confirm` 换掉。
 */
export function Modal({
  open,
  title,
  onClose,
  confirm,
  children,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  confirm?: { label: string; onClick: () => void; disabled?: boolean };
  children: ReactNode;
}) {
  const action = confirm ?? { label: "知道了", onClick: onClose };

  return (
    <Dialog
      open={open}
      ariaLabel={title}
      onOpenChange={(next) => (next ? undefined : onClose())}
    >
      <div className="flex flex-col">
        <header className="border-b-2 border-ink px-4 py-2.5 pr-12">
          <h3 className="font-display text-head font-bold text-ink">{title}</h3>
        </header>
        <div className="flex flex-col gap-2.5 px-4 py-3.5 text-body leading-relaxed text-ink">
          {children}
        </div>
        <footer className="flex justify-end border-t border-rule px-4 py-3">
          <Button size="sm" onClick={action.onClick} disabled={action.disabled}>
            {action.label}
          </Button>
        </footer>
      </div>
    </Dialog>
  );
}
