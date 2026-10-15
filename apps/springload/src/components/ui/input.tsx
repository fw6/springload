import { useId, type ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/utils";

export interface InputProps
  extends Omit<ComponentPropsWithoutRef<"input">, "value" | "defaultValue" | "onChange"> {
  label?: string;
  value?: string;
  defaultValue?: string;
  /** 每次输入都把字符串交回去，不做数字转换。 */
  onChange?: (value: string) => void;
  /** 出错时把边框与说明换成套红；传字符串时把这句话显示在下方。 */
  error?: string | boolean;
}

/** 输入框：方角、1px 油墨边，像版面上待填的一栏。 */
export function Input({
  label,
  value,
  defaultValue,
  onChange,
  error,
  className,
  id,
  ...rest
}: InputProps) {
  const generated = useId();
  const inputId = id ?? generated;
  const message = typeof error === "string" ? error : null;

  return (
    <div className={cn("flex min-w-0 flex-col gap-1", className)}>
      {label !== undefined ? (
        <label htmlFor={inputId} className="text-note font-medium text-ink-soft">
          {label}
        </label>
      ) : null}
      <input
        id={inputId}
        value={value}
        defaultValue={defaultValue}
        onChange={onChange === undefined ? undefined : (event) => onChange(event.target.value)}
        aria-invalid={error ? true : undefined}
        className={cn(
          "h-8.5 w-full border bg-slip px-2.5 text-body tabular text-ink",
          "placeholder:text-ink-soft",
          "focus:border-proof focus:outline-none",
          "disabled:cursor-not-allowed disabled:opacity-50",
          "read-only:text-ink-soft",
          error ? "border-alert bg-alert-soft" : "border-rule-strong",
        )}
        {...rest}
      />
      {message !== null ? <p className="text-note text-alert">{message}</p> : null}
    </div>
  );
}
