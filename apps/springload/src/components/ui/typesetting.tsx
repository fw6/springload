import { cn } from "../../lib/utils";

/**
 * 排版中的文字：字后面跟着一枚来回走的墨块，表示这一段还在印。
 * 不改变文字本身，读屏读到的还是那句话。
 */
export function Typesetting({
  children,
  className,
}: {
  children: string;
  className?: string;
}) {
  return (
    <span className={cn("inline-flex items-center gap-2 text-ink-soft", className)}>
      {children}
      <span aria-hidden data-slot="typesetting-bar" className="inline-flex gap-0.5">
        <span className="size-1 animate-pulse bg-current" />
        <span className="size-1 animate-pulse bg-current [animation-delay:160ms]" />
        <span className="size-1 animate-pulse bg-current [animation-delay:320ms]" />
      </span>
    </span>
  );
}
