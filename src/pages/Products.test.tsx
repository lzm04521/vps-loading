import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "antd";
import { I18nextProvider } from "react-i18next";
import i18next from "i18next";
import zhCN from "../i18n/zh-CN.json";
import type { VendorStat } from "../types";
import { productsList, regionsList, vendorsList } from "../api/ipc";
import Products from "./Products";

// 保留真实模块的其余导出（错误提示的格式化要用到 IpcError），只把 IPC 调用函数换成桩
vi.mock("../api/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api/ipc")>()),
  productsList: vi.fn(),
  vendorsList: vi.fn(),
  regionsList: vi.fn(),
  watchToggle: vi.fn(),
}));
vi.mock("../api/external", () => ({ openExternal: vi.fn() }));

const vendors: VendorStat[] = [
  { key: "yunyoo", name: "云悠", logo: "", total: 57, ok: 34 },
  { key: "vmiss", name: "VMISS", logo: "", total: 11, ok: 0 },
  { key: "dmit", name: "DMIT", logo: "", total: 2, ok: 2 },
];

beforeEach(() => {
  // 总数 60：每页 24 条，共 3 页
  vi.mocked(productsList).mockReset().mockResolvedValue({ total: 60, items: [] });
  vi.mocked(vendorsList).mockReset().mockResolvedValue(vendors);
  vi.mocked(regionsList).mockReset().mockResolvedValue([]);
});

/** 渲染库存页：页面用到 antd 的全局提示与多语言，需要两层 Provider */
function renderPage() {
  const i18n = i18next.createInstance();
  void i18n.init({
    lng: "zh-CN",
    resources: { "zh-CN": { translation: zhCN } },
    interpolation: { escapeValue: false },
  });
  return render(
    <App>
      <I18nextProvider i18n={i18n}>
        <Products />
      </I18nextProvider>
    </App>,
  );
}

/** 等到最近一次库存查询带上给定条件 */
const lastQueryHas = (query: object) =>
  waitFor(() => expect(productsList).toHaveBeenLastCalledWith(expect.objectContaining(query)));

describe("Products", () => {
  it("未选商家时不带商家条件", async () => {
    renderPage();
    await screen.findByText("共 60 款");
    expect(productsList).toHaveBeenLastCalledWith(
      expect.objectContaining({ vendors: null, offset: 0 }),
    );
  });

  it("勾选商家后带商家条件并回到第一页", async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("共 60 款");
    await user.click(screen.getByTitle("2")); // 分页条第 2 页
    await lastQueryHas({ offset: 24 });
    await user.click(await screen.findByRole("checkbox", { name: "云悠" }));
    // 结果集变小后回到第 1 页，避免当前页越界
    await lastQueryHas({ vendors: ["yunyoo"], offset: 0 });
  });

  it("点「全部」后恢复不带商家条件", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("checkbox", { name: "云悠" }));
    await lastQueryHas({ vendors: ["yunyoo"] });
    await user.click(screen.getByRole("radio", { name: /全部 \(ALL\)/ }));
    await lastQueryHas({ vendors: null });
  });

  it("收起侧栏不触发重新查询，展开后已选保持", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("checkbox", { name: "云悠" }));
    await lastQueryHas({ vendors: ["yunyoo"] });
    vi.mocked(productsList).mockClear();
    await user.click(screen.getByRole("button", { name: "收起" }));
    expect(screen.getByRole("button", { name: "展开" })).toBeInTheDocument();
    expect(productsList).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "展开" }));
    expect(screen.getByRole("checkbox", { name: "云悠" })).toBeChecked();
    expect(productsList).not.toHaveBeenCalled();
  });

  it("顶部筛选条不再有商家下拉", async () => {
    const { container } = renderPage();
    await screen.findByText("共 60 款");
    // 按占位文字判断，不数下拉框总数：分页条的“每页条数”也是一个下拉
    const placeholders = Array.from(
      container.querySelectorAll(".ant-select-selection-placeholder"),
    ).map((e) => e.textContent);
    expect(placeholders).not.toContain("商家");
  });

  it("商家列表加载失败时给出提示，主列表照常", async () => {
    vi.mocked(vendorsList).mockRejectedValue(new Error("商家列表挂了"));
    renderPage();
    expect(await screen.findByText("共 60 款")).toBeInTheDocument();
    // 失败不能悄悄吞掉：用户要看到提示，而不是一个只有「全部」的空侧栏
    expect(await screen.findByText("商家列表挂了")).toBeInTheDocument();
  });

  it("地区列表加载失败不影响商家列表", async () => {
    vi.mocked(regionsList).mockRejectedValue(new Error("地区列表挂了"));
    renderPage();
    expect(await screen.findByRole("checkbox", { name: "云悠" })).toBeInTheDocument();
    expect(await screen.findByText("地区列表挂了")).toBeInTheDocument();
  });
});
