// 与 Rust 端 DTO 镜像的类型定义（唯一类型源；后端 src-tauri/src/model.rs 为契约方，
// 跨边界一律 camelCase）。改这里必须同步改 Rust。

export type EventType =
  | "restock"
  | "soldout"
  | "price_down"
  | "price_up"
  | "new_product";

export const ALL_EVENT_TYPES: EventType[] = [
  "restock",
  "soldout",
  "price_down",
  "price_up",
  "new_product",
];

export type Channel = "local" | "telegram" | "webhook";

export interface AppSettings {
  autoStart: boolean;
  language: string;
  logLevel: string;
}

export interface Rule {
  id: number;
  name: string;
  enabled: boolean;
  /** 逗号分隔的事件类型串（沿用旧库存储格式） */
  eventTypes: string;
  vendorKey: string | null;
  regionLike: string | null;
  nameLike: string | null;
  routeLike: string | null;
  minPriceUsd: number | null;
  maxPriceUsd: number | null;
  onlyWatchlist: boolean;
  channel: string;
  cooldownMinutes: number;
  createdAt: string;
}

export interface RuleInput {
  name: string;
  enabled: boolean;
  eventTypes: string;
  vendorKey: string | null;
  regionLike: string | null;
  nameLike: string | null;
  routeLike: string | null;
  minPriceUsd: number | null;
  maxPriceUsd: number | null;
  onlyWatchlist: boolean;
  channel: string;
  cooldownMinutes: number;
}

export interface ProductQuery {
  status: string;
  vendor: string | null;
  region: string | null;
  q: string | null;
  minPrice: number | null;
  maxPrice: number | null;
  watch: boolean;
  sort: string;
  limit: number;
  offset: number;
}

export interface ProductView {
  id: number;
  vendorKey: string;
  vendorName: string;
  seriesKey: string;
  seriesName: string;
  name: string;
  price: string;
  priceUsd: number | null;
  region: string;
  regionCn: string;
  regionFlag: string;
  unit: string;
  spec: Record<string, unknown>;
  route: string;
  ipAttr: string;
  promo: string;
  promoExp: string;
  buyUrl: string;
  buyMode: string;
  blogUrl: string;
  orderText: string;
  status: boolean;
  firstSeen: string;
  lastSeen: string;
  lastChange: string | null;
  watched: boolean;
}

export interface ProductPage {
  total: number;
  items: ProductView[];
}

export interface VendorStat {
  key: string;
  name: string;
  logo: string;
  total: number;
  ok: number;
}

export interface RegionStat {
  regionCn: string;
  total: number;
  ok: number;
}

export interface EventView {
  id: number;
  productId: number;
  type: string;
  oldStatus: number | null;
  newStatus: number | null;
  oldPriceUsd: number | null;
  newPriceUsd: number | null;
  createdAt: string;
  productName: string;
  vendorKey: string;
  seriesName: string;
  currentPrice: string;
  currentPriceUsd: number | null;
  buyUrl: string;
  regionCn: string;
  vendorName: string;
}

export interface PollView {
  id: number;
  fetchedAt: string;
  httpStatus: number | null;
  changed: boolean;
  total: number | null;
  ok: number | null;
  oos: number | null;
  durationMs: number | null;
  error: string | null;
}

export interface AlertView {
  id: number;
  eventId: number;
  ruleId: number;
  productId: number;
  channel: string;
  message: string;
  sentAt: string;
  status: string;
  error: string | null;
  ruleName: string | null;
  productName: string;
  vendorKey: string;
  price: string;
  buyUrl: string;
  vendorName: string;
  regionCn: string;
}

export interface HistoryView {
  id: number;
  timeText: string;
  occurredAt: string | null;
  name: string;
  price: string;
  buyUrl: string;
}

export interface Counts {
  total: number;
  ok: number;
  oos: number;
  watched: number;
  events24h: number;
  alerts24h: number;
}

export interface SummaryView {
  counts: Counts;
  lastPoll: PollView | null;
  lastSuccess: string | null;
  lastChecked: string | null;
  lastDataChange: string | null;
  lastError: string;
  sourceUrl: string;
  initialized: boolean;
  collectorBusy: boolean;
  pollIntervalSeconds: number;
  telegramConfigured: boolean;
  webhookConfigured: boolean;
  dbPath: string;
}

export interface CollectResult {
  ok: boolean;
  skipped: boolean;
  notModified: boolean;
  changed: boolean;
  httpStatus: number | null;
  fetchedAt: string | null;
  durationMs: number;
  total: number | null;
  okCount: number | null;
  oosCount: number | null;
  added: number | null;
  removed: number | null;
  events: number | null;
  error: string | null;
}

export interface ConfigView {
  sourceUrl: string;
  pollIntervalSeconds: number;
  telegramChatId: string;
  telegramTokenSet: boolean;
  webhookUrl: string;
  settings: AppSettings;
  configPath: string;
  dbPath: string;
}

export interface ConfigInput {
  sourceUrl?: string;
  pollIntervalSeconds?: number;
  telegramChatId?: string;
  /** Some("") 表示清除；undefined 表示不改 */
  telegramToken?: string;
  webhookUrl?: string;
  settings?: AppSettings;
}

export interface LegacyImportReport {
  configImported: boolean;
  telegramTokenImported: boolean;
  dbImported: boolean;
  sourceUrl: string | null;
  detail: string[];
}

/** 后端 monitor://status 事件载荷 */
export interface StatusEvent {
  state: "running" | "done" | "error" | "skipped" | string;
  message: string;
  at: string;
}
