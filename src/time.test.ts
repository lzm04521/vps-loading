import { describe, expect, it } from "vitest";
import { formatDateTime, formatRelative } from "./time";

describe("formatRelative", () => {
  const now = Date.parse("2026-10-20T12:00:00+00:00");

  it("刚刚 / 分钟 / 小时 / 天", () => {
    expect(formatRelative("2026-10-20T11:59:30+00:00", now)).toBe("刚刚");
    expect(formatRelative("2026-10-20T11:45:00+00:00", now)).toBe("15 分钟前");
    expect(formatRelative("2026-10-20T09:00:00+00:00", now)).toBe("3 小时前");
    expect(formatRelative("2026-10-18T12:00:00+00:00", now)).toBe("2 天前");
  });

  it("空值与非法输入", () => {
    expect(formatRelative(null)).toBe("-");
    expect(formatRelative(undefined)).toBe("-");
    expect(formatRelative("not-a-date", now)).toBe("not-a-date");
  });

  it("超过 30 天退化为完整时间", () => {
    const out = formatRelative("2026-01-01T00:00:00+00:00", now);
    expect(out).not.toContain("天前");
    expect(out).not.toBe("-");
  });
});

describe("formatDateTime", () => {
  it("ISO 字符串可解析", () => {
    const out = formatDateTime("2026-10-20T12:00:00+00:00");
    expect(out).toMatch(/2026/);
  });

  it("空值与非法输入", () => {
    expect(formatDateTime(null)).toBe("-");
    expect(formatDateTime("garbage")).toBe("garbage");
  });
});
