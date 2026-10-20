import { describe, expect, it } from "vitest";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhCN from "./zh-CN.json";
import en from "./en.json";
import { IpcError } from "../api/ipc";
import { ipcErrorMessage } from "./errors";

void i18n.use(initReactI18next).init({
  resources: {
    "zh-CN": { translation: zhCN },
    en: { translation: en },
  },
  lng: "zh-CN",
  interpolation: { escapeValue: false },
});

describe("ipcErrorMessage", () => {
  it("有错误码时走 i18n 并插值 detail", () => {
    const err = new IpcError("RULE_INVALID", "未知事件类型：bogus");
    expect(ipcErrorMessage(err, i18n.t)).toBe("规则参数不合法：未知事件类型：bogus");
  });

  it("无 detail 插值的错误码也能翻译", () => {
    const err = new IpcError("RULE_NOT_FOUND", "规则不存在");
    expect(ipcErrorMessage(err, i18n.t)).toBe("规则不存在");
  });

  it("未知错误码回退到 detail 原文", () => {
    const err = new IpcError("SOME_FUTURE_CODE", "boom");
    expect(ipcErrorMessage(err, i18n.t)).toBe("boom");
  });

  it("无错误码时展示 detail", () => {
    const err = new IpcError(undefined, "raw backend string");
    expect(ipcErrorMessage(err, i18n.t)).toBe("raw backend string");
  });

  it("普通 Error 展示 message", () => {
    expect(ipcErrorMessage(new Error("network"), i18n.t)).toBe("network");
  });

  it("英文语言包同错误码有对应文案", async () => {
    await i18n.changeLanguage("en");
    const err = new IpcError("RULE_INVALID", "bad types");
    expect(ipcErrorMessage(err, i18n.t)).toBe("Invalid rule: bad types");
    await i18n.changeLanguage("zh-CN");
  });
});

describe("IpcError 解析", () => {
  it("后端 JSON 错误串能被解出 code（模拟 unwrap 逻辑）", () => {
    // 与 ipc.ts unwrap 相同的解析路径，直接验证 JSON 形态
    const raw = '{"code":"LEGACY_DB_CONFLICT","detail":"当前库已有 3 条产品数据"}';
    const parsed = JSON.parse(raw.match(/\{[\s\S]*\}/)![0]) as { code?: string; detail?: string };
    expect(parsed.code).toBe("LEGACY_DB_CONFLICT");
    expect(parsed.detail).toContain("3 条");
  });
});
