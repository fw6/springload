import { X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { usePresence } from "../../lib/hooks/use-presence";
import { useReducedMotion } from "../../lib/hooks/use-reduced-motion";
import { cn } from "../../lib/utils";

/** 压印时长，与 index.css 里 `.dialog-in` / `.dialog-out` 的时长一致。 */
const IMPRESS_MS = 260;

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 读屏念出的名字，通常就是弹窗标题。 */
  ariaLabel: string;
  children: ReactNode;
}

/** 弹窗：走原生 <dialog> 的顶层堆叠与焦点圈闭，面板从正中压印上来。 */
export function Dialog({ open, onOpenChange, ariaLabel, children }: DialogProps) {
  return (
    <DialogContent open={open} onOpenChange={onOpenChange} ariaLabel={ariaLabel}>
      {children}
    </DialogContent>
  );
}

export interface DialogContentProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 读屏念出的名字。 */
  ariaLabel?: string;
  dismissible?: boolean;
  showCloseButton?: boolean;
  closeButtonLabel?: string;
  className?: string;
  children: ReactNode;
}

export function DialogContent({
  open,
  onOpenChange,
  ariaLabel,
  dismissible = true,
  showCloseButton = true,
  closeButtonLabel = "关闭",
  className,
  children,
}: DialogContentProps) {
  const reduce = useReducedMotion();
  const [mounted, setMounted] = useState(false);
  const [dialog, setDialog] = useState<HTMLDialogElement | null>(null);

  // 关闭：面板留着放完收起动画再摘掉；要求减少动效时没有动画，直接摘。
  const present = usePresence(open, reduce ? 0 : IMPRESS_MS);

  useEffect(() => setMounted(true), []);

  // 面板挂上之后进顶层。摘面板由 React 移除 <dialog> 完成，那时浏览器自己会退出顶层。
  useEffect(() => {
    if (dialog === null || !open) return;
    if (!dialog.open) dialog.showModal();
  }, [dialog, open]);

  // 浏览器自己关掉对话框（Esc、点遮罩）时，把状态同步回 React。
  useEffect(() => {
    if (dialog === null) return;
    const onClose = () => onOpenChange(false);
    const onCancel = (event: Event) => {
      if (!dismissible) event.preventDefault();
    };
    dialog.addEventListener("close", onClose);
    dialog.addEventListener("cancel", onCancel);
    return () => {
      dialog.removeEventListener("close", onClose);
      dialog.removeEventListener("cancel", onCancel);
    };
  }, [dialog, dismissible, onOpenChange]);

  // Safari 还没有 closedby：点遮罩时手动关闭，判据是点击落在对话框内容盒之外。
  useEffect(() => {
    if (dialog === null || !dismissible) return;
    if ("closedBy" in HTMLDialogElement.prototype) return;
    const onClick = (event: MouseEvent) => {
      if (event.target !== dialog) return;
      const rect = dialog.getBoundingClientRect();
      const inside =
        event.clientY >= rect.top &&
        event.clientY <= rect.bottom &&
        event.clientX >= rect.left &&
        event.clientX <= rect.right;
      if (inside) return;
      dialog.close();
    };
    dialog.addEventListener("click", onClick);
    return () => dialog.removeEventListener("click", onClick);
  }, [dialog, dismissible]);

  if (!mounted || !present) return null;

  return createPortal(
    <dialog
      ref={setDialog}
      // React 还不认识 closedby，按自定义属性原样写下去。
      {...({ closedby: dismissible ? "any" : "none" } as Record<string, string>)}
      aria-label={ariaLabel}
      data-slot="dialog"
      className="m-auto w-[calc(100vw-2rem)] max-w-[30rem] border-0 bg-transparent p-0 text-ink backdrop:bg-ink/25"
    >
      <div
        data-state={open ? "open" : "closed"}
        className={cn(
          "relative max-h-[85vh] overflow-auto border-2 border-ink bg-paper",
          open ? "dialog-in" : "dialog-out",
          className,
        )}
      >
        {children}
        {showCloseButton ? (
          <button
            type="button"
            aria-label={closeButtonLabel}
            onClick={() => onOpenChange(false)}
            className="absolute top-2.5 right-2.5 inline-flex size-7 items-center justify-center border border-rule-strong text-ink-soft transition-colors hover:border-ink hover:bg-ink hover:text-paper"
          >
            <X aria-hidden size={14} strokeWidth={2} />
          </button>
        ) : null}
      </div>
    </dialog>,
    document.body,
  );
}
