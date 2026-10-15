import { ChevronUp } from "lucide-react";
import type { ComponentPropsWithoutRef, CSSProperties, ReactNode, Ref } from "react";
import { cn } from "../lib/utils";
import { Tooltip } from "./Tooltip";

/**
 * 行情表：上下各一条粗线夹住，行与行之间只隔一条细线，没有圆角也没有外框阴影。
 * 表头吸顶；选中的整行反白。
 *
 * `ref` 指向滚动视口，窗口化渲染的表格靠它读到当前滚到哪儿了。
 * `rowCount` 是表格真实的行数（含表头）：窗口化时 DOM 里只有视口附近那几行，
 * 这个数字让辅助技术知道总共多少行。
 */
export function DataTable({
  children,
  className,
  style,
  rowCount,
  ref,
}: {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  rowCount?: number;
  ref?: Ref<HTMLDivElement>;
}) {
  return (
    <div
      ref={ref}
      style={style}
      className={cn("overflow-auto border-y-2 border-ink bg-paper", className)}
    >
      <table aria-rowcount={rowCount} className="w-full border-collapse text-body">
        {children}
      </table>
    </div>
  );
}

const ALIGN = {
  left: "text-left",
  center: "text-center",
  right: "text-right",
} as const;

export type Align = keyof typeof ALIGN;

export function Th({
  children,
  align = "left",
  className,
}: {
  children?: ReactNode;
  align?: Align;
  className?: string;
}) {
  return (
    <th
      scope="col"
      className={cn(
        "sticky top-0 z-10 border-b border-ink bg-slip px-3 py-1.5 text-note font-medium text-ink-soft",
        ALIGN[align],
        className,
      )}
    >
      {children}
    </th>
  );
}

export function Td({
  children,
  align = "left",
  className,
  ...rest
}: {
  children?: ReactNode;
  align?: Align;
} & ComponentPropsWithoutRef<"td">) {
  return (
    <td className={cn("px-3 py-1.5 tabular", ALIGN[align], className)} {...rest}>
      {children}
    </td>
  );
}

export function Tr({
  children,
  selected,
  onClick,
  className,
  ref,
  ...rest
}: {
  children: ReactNode;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
  ref?: Ref<HTMLTableRowElement>;
} & ComponentPropsWithoutRef<"tr">) {
  return (
    <tr
      ref={ref}
      data-selected={selected}
      onClick={onClick}
      className={cn(
        "border-b border-rule-soft last:border-b-0",
        onClick !== undefined && "cursor-pointer",
        "hover:bg-slip",
        "data-[selected=true]:bg-ink data-[selected=true]:text-paper data-[selected=true]:[&>td]:text-paper data-[selected=true]:hover:bg-ink",
        className,
      )}
      {...rest}
    >
      {children}
    </tr>
  );
}

/** 可点击排序的列头。按钮的无障碍名是「<列名>排序」。 */
export function SortButton({
  label,
  tooltip,
  active,
  desc,
  onClick,
}: {
  label: string;
  tooltip: string;
  active: boolean;
  desc: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip text={tooltip}>
      <button
        type="button"
        aria-label={`${label}排序`}
        onClick={onClick}
        className={cn(
          "inline-flex items-center gap-1 transition-colors hover:text-ink",
          active && "font-semibold text-ink",
        )}
      >
        {label}
        <ChevronUp
          aria-hidden
          size={13}
          strokeWidth={2}
          className={cn(
            "shrink-0 transition-transform",
            active ? "opacity-100" : "opacity-40",
            active && !desc && "rotate-180",
          )}
        />
      </button>
    </Tooltip>
  );
}
