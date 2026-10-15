import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { invoke } from "../api/invoke";

/** 一个术语。 */
export interface Term {
  key: string;
  label: string;
  plain: string;
}

/** 一步的文案。 */
export interface StepCopy {
  index: number;
  title: string;
  goal: string;
  oneLine: string;
  detail: string;
  termKeys: string[];
  blockedHint: string;
}

/** `glossary` 命令的返回。 */
export interface Glossary {
  terms: Term[];
  steps: StepCopy[];
}

interface GlossaryValue {
  terms: Map<string, Term>;
  steps: StepCopy[];
  loaded: boolean;
  error: string | null;
}

const EMPTY: GlossaryValue = { terms: new Map(), steps: [], loaded: false, error: null };

const GlossaryContext = createContext<GlossaryValue>(EMPTY);

/** 启动时取一次 `glossary` 命令，把术语与四步文案放进 context。 */
export function GlossaryProvider({ children }: { children: ReactNode }) {
  const [value, setValue] = useState<GlossaryValue>(EMPTY);

  useEffect(() => {
    let alive = true;
    invoke<Glossary>("glossary")
      .then((glossary) => {
        if (!alive) return;
        setValue({
          terms: new Map(glossary.terms.map((term) => [term.key, term])),
          steps: glossary.steps,
          loaded: true,
          error: null,
        });
      })
      .catch((error: unknown) => {
        if (!alive) return;
        setValue({
          terms: new Map(),
          steps: [],
          loaded: true,
          error: error instanceof Error ? error.message : String(error),
        });
      });
    return () => {
      alive = false;
    };
  }, []);

  return <GlossaryContext value={value}>{children}</GlossaryContext>;
}

/** 取术语与四步文案。 */
export function useGlossary(): GlossaryValue {
  return useContext(GlossaryContext);
}

/** 按 key 取一个术语。 */
export function useTerm(key: string): Term | undefined {
  return useContext(GlossaryContext).terms.get(key);
}

/** 取四步文案。 */
export function useSteps(): StepCopy[] {
  return useContext(GlossaryContext).steps;
}
