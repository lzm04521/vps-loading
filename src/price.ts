// 价格展示统一拼接：price 自带货币符号（如 ¥36.99），unit 为源站周期（月/月付/年/季/半年）。
// 语义对齐后端 alerts.rs 的拼接容错，另加展示层归一化（去尾"付"字）。

/** 去掉 unit 尾部"付"字：月付→月、年付→年，统一显示格式（仅影响展示，不改存储与告警） */
function normalizeUnit(unit: string): string {
  return unit.endsWith("付") ? unit.slice(0, -1) : unit;
}

/**
 * 价格与单位拼接。规则：
 * - unit 或 price 为空 → 原样返回 price（调用方对空 price 回退美元折算价，折算参考价不带周期语义）
 * - price 已以 unit 结尾 → 不重复拼（与 alerts.rs 一致）
 * - 其余 → `price/归一化unit`，如 `¥36.99/月`、`¥99/年`
 */
export function priceWithUnit(price: string, unit: string): string {
  if (!unit || !price || price.endsWith(unit)) return price;
  return `${price}/${normalizeUnit(unit)}`;
}
