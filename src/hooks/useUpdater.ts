import { useCallback, useState } from "react";

// 私有仓库限制：Release 资产（latest.json）匿名不可下载，应用内自动更新默认停用。
// 本文件保留完整状态机结构；启用时置 UPDATER_ENABLED=true 并在 tauri.conf.json
// 配置 updater 端点（需仓库转公开或提供可匿名访问的 latest.json）。
export const UPDATER_ENABLED = false;

export type UpdaterState =
  | "idle"
  | "checking"
  | "available"
  | "uptodate"
  | "disabled"
  | "error";

export function useUpdater() {
  const [state, setState] = useState<UpdaterState>("idle");
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const check = useCallback(async () => {
    if (!UPDATER_ENABLED) {
      setState("disabled");
      return;
    }
    setState("checking");
    setError(null);
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        setVersion(update.version);
        setState("available");
      } else {
        setState("uptodate");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setState("error");
    }
  }, []);

  return { state, version, error, check, enabled: UPDATER_ENABLED };
}
