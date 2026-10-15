import { Check } from "lucide-react";
import { cn } from "../../lib/utils";

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  ariaLabel?: string;
  className?: string;
}

/** 开关：一枚方格，打开时里面打上油墨的叉。 */
export function Switch({
  checked,
  onCheckedChange,
  disabled = false,
  label,
  ariaLabel,
  className,
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel ?? label}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "inline-flex items-center gap-2 text-body text-ink disabled:opacity-45",
        className,
      )}
    >
      <span
        aria-hidden
        data-slot="switch-box"
        className={cn(
          "inline-flex size-4 shrink-0 items-center justify-center border",
          checked ? "border-ink bg-ink text-paper" : "border-rule-strong bg-slip",
        )}
      >
        {checked ? <Check size={12} strokeWidth={2} /> : null}
      </span>
      {label !== undefined ? <span>{label}</span> : null}
    </button>
  );
}
