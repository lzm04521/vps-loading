import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import i18next from "i18next";
import zhCN from "../i18n/zh-CN.json";
import type { VendorStat } from "../types";
import { VendorFilter } from "./VendorFilter";

function setup() {
  const i18n = i18next.createInstance();
  void i18n.init({
    lng: "zh-CN",
    resources: { "zh-CN": { translation: zhCN } },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

const vendors: VendorStat[] = [
  { key: "yunyoo", name: "云悠", logo: "", total: 57, ok: 34 },
  { key: "vmiss", name: "VMISS", logo: "", total: 11, ok: 0 },
  { key: "dmit", name: "DMIT", logo: "", total: 2, ok: 2 },
];

/** 渲染商家筛选栏：返回回调桩，以及“换一组已选商家后重渲染”的方法 */
function renderFilter(selected: string[], list: VendorStat[] = vendors) {
  const i18n = setup();
  const onChange = vi.fn();
  const view = (sel: string[]) => (
    <I18nextProvider i18n={i18n}>
      <VendorFilter vendors={list} selected={sel} onChange={onChange} />
    </I18nextProvider>
  );
  const { rerender } = render(view(selected));
  return { onChange, select: (sel: string[]) => rerender(view(sel)) };
}

const allRow = () => screen.getByRole("radio", { name: /全部 \(ALL\)/ });
const vendorCheckbox = (name: string) => screen.getByRole("checkbox", { name });

describe("VendorFilter", () => {
  it("未选商家时「全部」行选中并显示有货与总数汇总", () => {
    renderFilter([]);
    expect(allRow()).toBeChecked();
    // 有货 34+0+2、总数 57+11+2
    expect(screen.getByText("36/70")).toBeInTheDocument();
    expect(screen.getByText("34/57")).toHaveStyle({ color: "#52c41a" });
    expect(screen.getByText("0/11")).toHaveStyle({ color: "#999" });
  });

  it("勾选与取消勾选商家时回调新的已选列表", async () => {
    const user = userEvent.setup();
    const { onChange, select } = renderFilter([]);
    await user.click(vendorCheckbox("云悠"));
    expect(onChange).toHaveBeenLastCalledWith(["yunyoo"]);
    select(["yunyoo"]);
    expect(vendorCheckbox("云悠")).toBeChecked();
    expect(allRow()).not.toBeChecked();
    await user.click(vendorCheckbox("DMIT"));
    // 新勾选的商家追加在末尾
    expect(onChange).toHaveBeenLastCalledWith(["yunyoo", "dmit"]);
    select(["yunyoo", "dmit"]);
    await user.click(vendorCheckbox("云悠"));
    expect(onChange).toHaveBeenLastCalledWith(["dmit"]);
  });

  it("点「全部」清空已选", async () => {
    const { onChange } = renderFilter(["yunyoo", "dmit"]);
    await userEvent.setup().click(allRow());
    expect(onChange).toHaveBeenCalledExactlyOnceWith([]);
  });

  it("逐个勾满商家时保持各自选中，「全部」行不抢选中态", () => {
    renderFilter(["yunyoo", "vmiss", "dmit"]);
    for (const name of ["云悠", "VMISS", "DMIT"]) {
      expect(vendorCheckbox(name)).toBeChecked();
    }
    expect(allRow()).not.toBeChecked();
  });

  it("搜索按名称或标识过滤商家，已选数量仍显示", async () => {
    const user = userEvent.setup();
    renderFilter(["yunyoo"]);
    const search = screen.getByPlaceholderText("搜索商家");
    await user.type(search, "dmi"); // 命中标识 dmit
    expect(screen.queryByRole("checkbox", { name: "云悠" })).toBeNull();
    expect(vendorCheckbox("DMIT")).toBeInTheDocument();
    // 被搜索隐藏的已选商家仍计入标题旁的角标
    expect(screen.getByText("1")).toBeInTheDocument();
    await user.clear(search);
    await user.type(search, "云"); // 命中名称
    expect(vendorCheckbox("云悠")).toBeInTheDocument();
  });

  it("搜索无匹配时提示暂无数据且「全部」行保留", async () => {
    renderFilter([]);
    await userEvent.setup().type(screen.getByPlaceholderText("搜索商家"), "zzz");
    expect(screen.getByText("暂无数据")).toBeInTheDocument();
    expect(allRow()).toBeInTheDocument();
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  it("收起后只留展开入口与竖排标题，展开后已选保持", async () => {
    const user = userEvent.setup();
    const { onChange } = renderFilter(["yunyoo"]);
    await user.click(screen.getByRole("button", { name: "收起" }));
    expect(screen.queryByPlaceholderText("搜索商家")).toBeNull();
    expect(screen.getByRole("button", { name: "展开" })).toBeInTheDocument();
    expect(screen.getByText("商家筛选")).toBeInTheDocument();
    // 窄条上仍能看到已选数量
    expect(screen.getByText("1")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "展开" }));
    expect(screen.getByPlaceholderText("搜索商家")).toBeInTheDocument();
    expect(vendorCheckbox("云悠")).toBeChecked();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("商家列表为空时只显示「全部」行且不带计数", () => {
    renderFilter([], []);
    expect(allRow()).toBeInTheDocument();
    expect(screen.queryByText("0/0")).toBeNull();
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  it("已是全部时再点「全部」不回调", async () => {
    const { onChange } = renderFilter([]);
    await userEvent.setup().click(allRow());
    expect(onChange).not.toHaveBeenCalled();
  });
});
