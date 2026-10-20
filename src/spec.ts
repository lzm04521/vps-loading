// 库存卡片规格六宫格的取格逻辑：spec 键名自带 emoji 前缀（如 "💻 CPU"），
// 按固定优先级取满 6 格保证卡片统一大小，超出项供 Tooltip 展示。

export interface SpecEntry {
  icon: string;
  label: string;
  value: string;
}

export interface SpecPick {
  /** 展示用格位，最多 6 个 */
  entries: SpecEntry[];
  /** 第 7 项起（虚拟化/端口/备注等），hover Tooltip 展示 */
  extras: SpecEntry[];
}

export const SPEC_GRID_SIZE = 6;

/** 与图1 六宫格一致的展示顺序；IP地址/IP 为同义键名，都列出以兼容两种数据 */
const PRIORITY = ["CPU", "内存", "存储", "流量", "带宽", "IP地址", "IP"];

const KEY_RE =
  /^(\p{Extended_Pictographic}(?:\uFE0F|\u200D\p{Extended_Pictographic})*)\s*(.*)$/u;

function formatValue(v: unknown): string {
  if (v == null) return "-";
  if (typeof v === "string") {
    const s = v.trim();
    return s === "" ? "-" : s;
  }
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  return JSON.stringify(v);
}

export function pickSpecEntries(
  spec: Record<string, unknown> | null | undefined,
): SpecPick {
  const all: SpecEntry[] = Object.entries(spec ?? {})
    .filter(([k]) => k.trim() !== "")
    .map(([k, v]) => {
      const m = KEY_RE.exec(k.trim());
      const icon = m?.[1] ?? "";
      const label = (m?.[2] ?? "").trim() || k.trim();
      return { icon, label, value: formatValue(v) };
    });

  const used = new Set<SpecEntry>();
  const picked: SpecEntry[] = [];
  for (const want of PRIORITY) {
    if (picked.length >= SPEC_GRID_SIZE) break;
    const hit = all.find((e) => e.label === want && !used.has(e));
    if (hit) {
      used.add(hit);
      picked.push(hit);
    }
  }
  for (const e of all) {
    if (picked.length >= SPEC_GRID_SIZE) break;
    if (!used.has(e)) {
      used.add(e);
      picked.push(e);
    }
  }
  return { entries: picked, extras: all.filter((e) => !used.has(e)) };
}
