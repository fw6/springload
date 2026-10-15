import { Check } from "lucide-react";
import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "../../lib/utils";

export type ButtonVariant = "primary" | "secondary" | "outline" | "ghost";
export type ButtonSize = "sm" | "md" | "lg" | "icon";

/**
 * 按钮：方角、1px 油墨边，像版面上的加框小启事。
 * 悬停时把底色与字色对调（印刷里的反白），按下时向下压 1px。
 */
const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "border-proof bg-proof text-paper hover:bg-ink hover:border-ink",
  secondary:
    "border-ink bg-slip text-ink hover:bg-ink hover:text-paper",
  outline:
    "border-ink bg-transparent text-ink hover:bg-ink hover:text-paper",
  ghost:
    "border-transparent bg-transparent text-ink-soft hover:text-ink hover:underline hover:decoration-dotted hover:underline-offset-4",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-7 px-2.5 text-note",
  md: "h-8.5 px-3.5 text-body",
  lg: "h-10 px-5 text-lede",
  icon: "h-7 w-7 p-0",
};

export interface ButtonProps extends ComponentPropsWithoutRef<"button"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

export function Button({
  variant = "primary",
  size = "md",
  className,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex shrink-0 items-center justify-center gap-1.5 border font-paper font-medium",
        "transition-colors duration-100 active:translate-y-px",
        "disabled:pointer-events-none disabled:opacity-40",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    />
  );
}

export type ButtonState = "idle" | "loading" | "success" | "error";

export interface StatefulButtonProps extends ButtonProps {
  state?: ButtonState;
  children: ReactNode;
  loadingText?: ReactNode;
  successText?: ReactNode;
  errorText?: ReactNode;
}

/** 有进行时的按钮：加载时左边滚一枚墨点，文案换成进行时。 */
export function StatefulButton({
  state = "idle",
  children,
  loadingText,
  successText,
  errorText,
  disabled,
  className,
  ...rest
}: StatefulButtonProps) {
  const label =
    state === "loading"
      ? (loadingText ?? children)
      : state === "success"
        ? (successText ?? children)
        : state === "error"
          ? (errorText ?? children)
          : children;

  return (
    <Button
      disabled={disabled ?? state === "loading"}
      className={cn("relative", className)}
      {...rest}
    >
      {state === "loading" ? (
        <span
          aria-hidden
          data-slot="press-dot"
          className="size-1.5 shrink-0 animate-pulse bg-current"
        />
      ) : null}
      {state === "success" ? (
        <Check aria-hidden className="shrink-0" size={13} strokeWidth={2} />
      ) : null}
      {label}
    </Button>
  );
}
