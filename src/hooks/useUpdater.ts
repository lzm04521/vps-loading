import { useCallback, useRef, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// 更新源为公开仓库 Release 的 latest.json（tauri.conf.json plugins.updater），
// 安装包经 minisign 签名校验后安装。
export type UpdaterState =
  | "idle"
  | "checking"
  | "available"
  | "uptodate"
  | "downloading"
  | "ready"
  | "error";

export interface UpdateProgress {
  /** 已下载字节数（累计） */
  downloaded: number;
  /** 总字节数；服务端未返回 Content-Length 时为 null */
  total: number | null;
}

export function useUpdater() {
  const [state, setState] = useState<UpdaterState>("idle");
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const updateRef = useRef<Update | null>(null);

  /** 检查更新；返回是否存在新版本（失败置 error 态并返回 false） */
  const check = useCallback(async (): Promise<boolean> => {
    setState("checking");
    setError(null);
    setVersion(null);
    setProgress(null);
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      updateRef.current = update ?? null;
      if (update) {
        setVersion(update.version);
        setState("available");
        return true;
      }
      setState("uptodate");
      return false;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setState("error");
      return false;
    }
  }, []);

  /** 下载并安装当前检查到的新版本；完成后进入 ready 态等待重启 */
  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    setState("downloading");
    setError(null);
    setProgress({ downloaded: 0, total: null });
    let downloaded = 0;
    let total: number | null = null;
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? null;
            setProgress({ downloaded, total });
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            setProgress({ downloaded, total });
            break;
          case "Finished":
            break;
        }
      });
      setState("ready");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setState("error");
    }
  }, []);

  /** 重启应用以完成升级（Windows 下安装器也可能自动拉起新版本） */
  const restart = useCallback(() => relaunch(), []);

  return { state, version, error, progress, check, install, restart };
}
