import { describe, expect, it } from "vitest";
import zhCN from "./zh-CN.json";
import en from "./en.json";

/** 递归收集 "a.b.c" 形式的叶子键路径 */
function flattenKeys(obj: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([key, value]) =>
    typeof value === "object" && value !== null
      ? flattenKeys(value as Record<string, unknown>, `${prefix}${key}.`)
      : [`${prefix}${key}`],
  );
}

describe("i18n parity", () => {
  it("中英文键集合一致", () => {
    expect(flattenKeys(en).sort()).toEqual(flattenKeys(zhCN).sort());
  });
});
