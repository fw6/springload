import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  type ComponentPropsWithoutRef,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { cn } from "../../lib/utils";

interface TabsValue {
  value: string;
  select: (next: string) => void;
}

const TabsContext = createContext<TabsValue | null>(null);

function useTabs(component: string): TabsValue {
  const context = useContext(TabsContext);
  if (context === null) throw new Error(`${component} 必须放在 <Tabs> 里`);
  return context;
}

export interface TabsProps {
  value: string;
  onValueChange: (value: string) => void;
  className?: string;
  children: ReactNode;
}

/** 版次条：一次只选中一格，方向键在格与格之间走。 */
export function Tabs({ value, onValueChange, className, children }: TabsProps) {
  const context = useMemo(
    () => ({ value, select: onValueChange }),
    [value, onValueChange],
  );
  return (
    <TabsContext value={context}>
      <div className={className}>{children}</div>
    </TabsContext>
  );
}

export function TabsList({
  className,
  children,
  ...rest
}: ComponentPropsWithoutRef<"div">) {
  const listRef = useRef<HTMLDivElement | null>(null);

  const onKeyDown = useCallback((event: KeyboardEvent<HTMLDivElement>) => {
    const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const list = listRef.current;
    if (list === null) return;
    const tabs = Array.from(
      list.querySelectorAll<HTMLButtonElement>('[role="tab"]:not([disabled])'),
    );
    const current = tabs.findIndex((tab) => tab === document.activeElement);
    if (current < 0) return;
    event.preventDefault();
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? tabs.length - 1
          : event.key === "ArrowRight"
            ? (current + 1) % tabs.length
            : (current - 1 + tabs.length) % tabs.length;
    tabs[next]?.focus();
    tabs[next]?.click();
  }, []);

  return (
    <div
      ref={listRef}
      role="tablist"
      onKeyDown={onKeyDown}
      className={className}
      {...rest}
    >
      {children}
    </div>
  );
}

export interface TabsTriggerProps extends ComponentPropsWithoutRef<"button"> {
  value: string;
  /** 标成不可进入：仍能聚焦并读出原因，只是不会被选中。 */
  locked?: boolean;
}

export function TabsTrigger({
  value,
  locked = false,
  className,
  onClick,
  ...rest
}: TabsTriggerProps) {
  const { value: selected, select } = useTabs("TabsTrigger");
  const active = selected === value;

  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      aria-disabled={locked || undefined}
      tabIndex={active ? 0 : -1}
      className={cn("relative text-left", className)}
      onClick={(event) => {
        onClick?.(event);
        if (event.defaultPrevented || locked) return;
        select(value);
      }}
      {...rest}
    />
  );
}
