import { describe, expect, it } from "vitest";
import { priceWithUnit } from "./price";

describe("priceWithUnit", () => {
  it("常规拼接", () => {
    expect(priceWithUnit("¥36.99", "月")).toBe("¥36.99/月");
    expect(priceWithUnit("C$5.00", "季")).toBe("C$5.00/季");
    expect(priceWithUnit("$9.00", "半年")).toBe("$9.00/半年");
  });

  it("unit 为空原样返回", () => {
    expect(priceWithUnit("¥36.99", "")).toBe("¥36.99");
  });

  it("price 为空原样返回（调用方回退折算价，不带周期）", () => {
    expect(priceWithUnit("", "月")).toBe("");
  });

  it("去「付」归一化", () => {
    expect(priceWithUnit("¥36.99", "月付")).toBe("¥36.99/月");
    expect(priceWithUnit("¥99", "年付")).toBe("¥99/年");
    expect(priceWithUnit("¥15", "季付")).toBe("¥15/季");
  });

  it("price 已含 unit 不重复拼", () => {
    expect(priceWithUnit("¥36.99/月", "月")).toBe("¥36.99/月");
    expect(priceWithUnit("¥99/年付", "年付")).toBe("¥99/年付");
  });
});
