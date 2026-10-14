import { Check, Minus } from "lucide-react";
import { cn } from "../lib/utils";

/** 判定结果：满足打勾，不满足留一道横线。 */
export function Flag({ ok }: { ok: boolean }) {
  return (
    <span
      aria-hidden
      className={cn("inline-flex w-4 justify-center", ok ? "text-ok" : "text-rule")}
    >
      {ok ? <Check size={14} strokeWidth={2} /> : <Minus size={14} strokeWidth={2} />}
    </span>
  );
}
