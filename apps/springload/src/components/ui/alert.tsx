import { CircleCheck, Info, TriangleAlert } from "lucide-react";
import { createContext, useContext, type ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/utils";

export type AlertVariant = "default" | "info" | "proof" | "success" | "warning" | "destructive";

/** 每种告示的框线颜色与角标图形。 */
const TONES: Record<AlertVariant, { box: string; text: string }> = {
  default: { box: "border-rule", text: "text-ink-soft" },
  info: { box: "border-rule", text: "text-ink-soft" },
  proof: { box: "border-proof", text: "text-proof" },
  success: { box: "border-ok", text: "text-ok" },
  warning: { box: "border-warn", text: "text-warn" },
  destructive: { box: "border-alert", text: "text-alert" },
};

const MARKS: Record<AlertVariant, typeof Info> = {
  default: Info,
  info: Info,
  proof: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  destructive: TriangleAlert,
};

const AlertContext = createContext<AlertVariant>("default");

export interface AlertProps extends ComponentPropsWithoutRef<"div"> {
  variant?: AlertVariant;
}

/** 告示：整圈 1px 框线的小块，用来就地提醒。 */
export function Alert({ variant = "default", className, children, ...rest }: AlertProps) {
  const tone = TONES[variant];
  return (
    <AlertContext value={variant}>
      <div
        role={variant === "warning" || variant === "destructive" ? "alert" : "status"}
        data-slot="alert"
        data-variant={variant}
        className={cn("flex items-start gap-2 border bg-slip px-3 py-2.5 text-note", tone.box, className)}
        {...rest}
      >
        {children}
      </div>
    </AlertContext>
  );
}

export function AlertIcon({ className, children, ...rest }: ComponentPropsWithoutRef<"span">) {
  const variant = useContext(AlertContext);
  const tone = TONES[variant];
  const Mark = MARKS[variant];
  return (
    <span
      aria-hidden
      data-slot="alert-icon"
      className={cn("mt-px inline-flex shrink-0 items-center", tone.text, className)}
      {...rest}
    >
      {children ?? <Mark size={14} strokeWidth={2} />}
    </span>
  );
}

export function AlertContent({ className, ...rest }: ComponentPropsWithoutRef<"div">) {
  return <div data-slot="alert-content" className={cn("min-w-0 flex-1", className)} {...rest} />;
}

export function AlertDescription({ className, ...rest }: ComponentPropsWithoutRef<"div">) {
  return (
    <div
      data-slot="alert-description"
      className={cn("leading-relaxed text-ink", className)}
      {...rest}
    />
  );
}
