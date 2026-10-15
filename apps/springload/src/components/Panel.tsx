import type { ReactNode } from "react";
import { cn } from "../lib/utils";

/**
 * 稿件框：版面上一则加了框的稿件。
 * 标题一行，右侧留给动作按钮，下面一条粗线把题头与正文分开。
 */
export function Panel({
  title,
  action,
  children,
  className,
}: {
  title: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      data-slot="panel"
      className={cn("flex min-w-0 flex-col border border-rule bg-paper", className)}
    >
      <header className="flex flex-none flex-wrap items-baseline gap-x-3 gap-y-1 border-b-2 border-ink px-3.5 py-2.5">
        <h2 className="font-display text-head font-bold text-ink">{title}</h2>
        {action !== undefined ? (
          <div className="ml-auto flex flex-wrap items-center gap-2">{action}</div>
        ) : null}
      </header>
      <div className="flex flex-col gap-3.5 p-3.5">{children}</div>
    </section>
  );
}

/** 小节标题：栏线下面一行加粗的宋体。 */
export function SectionHead({ children }: { children: ReactNode }) {
  return (
    <h3 className="border-b border-rule pb-1 font-display text-body font-bold text-ink">
      {children}
    </h3>
  );
}

/**
 * 结果栏：项目在左、数值紧跟在右，逐行压在同一批栏线上。
 * 标签占固定的窄列，数值从同一条左边缘起排，一行读下来就是一组。
 */
export function StatGrid({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <dl
      data-slot="result-grid"
      className={cn("flex flex-col border-y-2 border-ink bg-slip px-3", className)}
    >
      {children}
    </dl>
  );
}

export function StatTile({
  label,
  value,
  hint,
}: {
  label: ReactNode;
  value: ReactNode;
  hint?: ReactNode;
}) {
  return (
    <div
      data-slot="result-cell"
      className="flex items-baseline gap-x-3 border-b border-rule-soft py-1.5 last:border-b-0"
    >
      <dt className="w-[8.5rem] shrink-0 text-note text-ink-soft">{label}</dt>
      <dd className="min-w-0 flex-1 font-mono text-head font-medium tabular text-ink">
        {value}
        {hint !== undefined ? <span className="ml-2 text-micro text-ink-soft">{hint}</span> : null}
      </dd>
    </div>
  );
}
