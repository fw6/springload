import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/** 命令调用失败的统一错误：带命令名与可展示文案。 */
export class CommandError extends Error {
  readonly command: string;

  constructor(command: string, message: string) {
    super(message);
    this.name = "CommandError";
    this.command = command;
  }
}

/** 把 Tauri 命令的错误值转成可展示文案。 */
export function toMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

/**
 * 统一的命令调用入口：错误一律转成 {@link CommandError}。
 * 命令名用 snake_case，入参用 camelCase 具名参数。
 */
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw new CommandError(command, toMessage(error));
  }
}
