import { openUrl } from "@tauri-apps/plugin-opener";

/** 调起系统默认浏览器打开外链（Tauri WebView 内 window.open / target=_blank 无效） */
export function openExternal(url: string): Promise<void> {
  return openUrl(url);
}
