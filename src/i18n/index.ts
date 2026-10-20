import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhCN from "./zh-CN.json";
import en from "./en.json";
import { invoke } from "@tauri-apps/api/core";

export const SUPPORTED_LANGUAGES = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
] as const;

void i18n.use(initReactI18next).init({
  resources: {
    "zh-CN": { translation: zhCN },
    en: { translation: en },
  },
  lng: "zh-CN",
  fallbackLng: "zh-CN",
  interpolation: { escapeValue: false },
});

/** 启动语言从全局设置读取（设置页保存后重启界面生效） */
export async function initLanguage(): Promise<void> {
  try {
    const config = await invoke<{ settings: { language: string } }>("config_get");
    const lang = config.settings.language;
    if (lang && lang !== i18n.language && (lang === "zh-CN" || lang === "en")) {
      await i18n.changeLanguage(lang);
    }
  } catch {
    // 拿不到设置时保持默认中文
  }
}
