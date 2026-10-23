import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AlertView,
  CollectResult,
  ConfigInput,
  ConfigView,
  EventView,
  HistoryView,
  LegacyImportReport,
  NotifyTestResult,
  PollView,
  ProductPage,
  ProductQuery,
  RegionStat,
  Rule,
  RuleInput,
  StatusEvent,
  SummaryView,
  VendorStat,
} from "../types";

// ── 错误结构化 ───────────────────────────────────────────────────────

/**
 * IPC 调用错误。后端命令失败返回 `{"code":"...","detail":"..."}` JSON 串；
 * code 供 i18n 按 errors.<code> 取文案，detail 为兜底。
 */
export class IpcError extends Error {
  readonly code?: string;
  readonly detail: string;

  constructor(code: string | undefined, detail: string) {
    super(detail);
    this.name = "IpcError";
    this.code = code;
    this.detail = detail;
  }
}

/** 解析后端错误串：能识别出 code 的包装为 IpcError，否则按原文包装（code=undefined） */
function unwrap<T>(p: Promise<T>): Promise<T> {
  return p.catch((raw: unknown) => {
    const text = typeof raw === "string" ? raw : String(raw);
    const jsonPart = text.match(/\{[\s\S]*\}/)?.[0];
    if (jsonPart) {
      try {
        const parsed = JSON.parse(jsonPart) as { code?: unknown; detail?: unknown };
        if (parsed && typeof parsed.code === "string") {
          const detail = typeof parsed.detail === "string" ? parsed.detail : text;
          return Promise.reject(new IpcError(parsed.code, detail));
        }
      } catch {
        // 非法 JSON：按原文展示
      }
    }
    return Promise.reject(new IpcError(undefined, text));
  });
}

// ── 总览 / 库存 ──────────────────────────────────────────────────────

export const summaryGet = () => unwrap(invoke<SummaryView>("summary_get"));

export const productsList = (query: Partial<ProductQuery>) =>
  unwrap(invoke<ProductPage>("products_list", { query }));

export const vendorsList = () => unwrap(invoke<VendorStat[]>("vendors_list"));

export const regionsList = () => unwrap(invoke<RegionStat[]>("regions_list"));

export const watchToggle = (productId: number) =>
  unwrap(invoke<boolean>("watch_toggle", { productId }));

// ── 事件 / 动态 / 告警 / 采集日志 ────────────────────────────────────

export const eventsList = (limit: number, eventType: string | null) =>
  unwrap(invoke<EventView[]>("events_list", { limit, eventType }));

export const sourceHistoryList = (limit: number) =>
  unwrap(invoke<HistoryView[]>("source_history_list", { limit }));

export const alertsList = (limit: number) =>
  unwrap(invoke<AlertView[]>("alerts_list", { limit }));

export const alertDelete = (id: number) =>
  unwrap(invoke<void>("alert_delete", { id }));

export const alertsClear = () => unwrap(invoke<number>("alerts_clear"));

export const pollsList = (limit: number) =>
  unwrap(invoke<PollView[]>("polls_list", { limit }));

// ── 规则 ─────────────────────────────────────────────────────────────

export const rulesList = () => unwrap(invoke<Rule[]>("rules_list"));

export const ruleCreate = (input: RuleInput) =>
  unwrap(invoke<Rule>("rule_create", { input }));

export const ruleUpdate = (id: number, input: RuleInput) =>
  unwrap(invoke<Rule>("rule_update", { id, input }));

export const ruleDelete = (id: number) => unwrap(invoke<void>("rule_delete", { id }));

// ── 采集 / 配置 / 导入 ───────────────────────────────────────────────

export const collectManual = (force: boolean) =>
  unwrap(invoke<CollectResult>("collect_manual", { force }));

export const notifyTest = (channel: string) =>
  unwrap(invoke<NotifyTestResult>("notify_test", { channel }));

export const configGet = () => unwrap(invoke<ConfigView>("config_get"));

export const configUpdate = (input: ConfigInput) =>
  unwrap(invoke<ConfigView>("config_update", { input }));

export const legacyImport = (dir: string, overwriteDb: boolean) =>
  unwrap(invoke<LegacyImportReport>("legacy_import", { dir, overwriteDb }));

export const appOpenLogDir = () => unwrap(invoke<void>("app_open_log_dir"));

// ── 事件订阅 ─────────────────────────────────────────────────────────

export function onStatusEvent(cb: (event: StatusEvent) => void) {
  return listen<StatusEvent>("monitor://status", (ev) => cb(ev.payload));
}
