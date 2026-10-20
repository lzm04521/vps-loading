import { describe, expect, it } from "vitest";
import { pickSpecEntries, type SpecEntry } from "./spec";

function labels(entries: SpecEntry[]): string[] {
  return entries.map((e) => e.label);
}

const STANDARD = {
  "🚀 带宽": "20Mbps",
  "🌐 IP地址": "IPv4",
  "💻 CPU": "2核",
  "🧠 内存": "2G",
  "💾 存储": "30GB",
  "📊 流量": "不限流量",
};

describe("pickSpecEntries", () => {
  it("按固定优先级排序，与图1 六宫格一致", () => {
    const { entries, extras } = pickSpecEntries(STANDARD);
    expect(labels(entries)).toEqual(["CPU", "内存", "存储", "流量", "带宽", "IP地址"]);
    expect(extras).toEqual([]);
    // emoji 剥离为图标，值原样保留
    expect(entries[0]).toEqual({ icon: "💻", label: "CPU", value: "2核" });
  });

  it("兼容 🌐 IP 变体键名", () => {
    const spec = { ...STANDARD, "🌐 IP": "IPv4" };
    delete (spec as Record<string, unknown>)["🌐 IP地址"];
    const { entries } = pickSpecEntries(spec);
    expect(labels(entries)).toEqual(["CPU", "内存", "存储", "流量", "带宽", "IP"]);
  });

  it("IP地址 与 IP 同现时优先取 IP地址，IP 不再入格", () => {
    const spec = { ...STANDARD, "🌐 IP": "IPv4" };
    const { entries, extras } = pickSpecEntries(spec);
    expect(labels(entries)).toContain("IP地址");
    expect(labels(entries)).not.toContain("IP");
    expect(labels(extras)).toContain("IP");
  });

  it("不足 6 格时保留命中项并用其余键按原顺序补齐", () => {
    const { entries } = pickSpecEntries({
      "💻 CPU": 1,
      "🧠 内存": "1GB",
      "🧩 虚拟化": "KVM",
    });
    expect(labels(entries)).toEqual(["CPU", "内存", "虚拟化"]);
    expect(entries[0].value).toBe("1");
  });

  it("超过 6 项时第 7 项起收入 extras", () => {
    const { entries, extras } = pickSpecEntries({
      ...STANDARD,
      "🧩 虚拟化": "KVM",
      "🔌 端口": "共享",
    });
    expect(entries).toHaveLength(6);
    expect(labels(extras)).toEqual(["虚拟化", "端口"]);
  });

  it("无 emoji 的键：icon 为空、label 为原文", () => {
    const { entries } = pickSpecEntries({ CPU: "1核", 机房: "东京" });
    const cpu = entries.find((e) => e.label === "CPU");
    expect(cpu?.icon).toBe("");
    expect(labels(entries)).toEqual(["CPU", "机房"]);
  });

  it("空值与缺省：空串/null 归一为 '-'，数字转字符串", () => {
    const { entries } = pickSpecEntries({ "💻 CPU": 2, "🧠 内存": "", "💾 存储": null });
    const byLabel = Object.fromEntries(entries.map((e) => [e.label, e.value]));
    expect(byLabel).toEqual({ CPU: "2", 内存: "-", 存储: "-" });
  });

  it("空 spec / null 安全返回空结果", () => {
    expect(pickSpecEntries({})).toEqual({ entries: [], extras: [] });
    expect(pickSpecEntries(null)).toEqual({ entries: [], extras: [] });
  });
});
