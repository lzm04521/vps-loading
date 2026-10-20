import type { TFunction } from "i18next";
import { IpcError } from "../api/ipc";

/** 把 IPC 错误映射为本地化文案：有错误码走 errors.<code>，否则展示原始 detail */
export function ipcErrorMessage(e: unknown, t: TFunction): string {
  if (e instanceof IpcError) {
    if (e.code) {
      const key = `errors.${e.code}`;
      const translated = t(key, { detail: e.detail });
      // i18n 缺键时返回 key 本身，此时退回 detail
      return translated === key ? e.detail : translated;
    }
    return e.detail;
  }
  return e instanceof Error ? e.message : String(e);
}
