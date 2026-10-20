import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import i18next from "i18next";
import zhCN from "../i18n/zh-CN.json";
import { EventTag } from "./EventTag";

function setup() {
  const i18n = i18next.createInstance();
  void i18n.init({
    lng: "zh-CN",
    resources: { "zh-CN": { translation: zhCN } },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

describe("EventTag", () => {
  it("已知事件类型显示本地化标签", () => {
    const i18n = setup();
    const { getByText, container } = render(
      <I18nextProvider i18n={i18n}>
        <EventTag type="restock" />
      </I18nextProvider>,
    );
    expect(getByText("补货")).toBeInTheDocument();
    // 对应颜色
    expect(container.querySelector(".ant-tag-green")).not.toBeNull();
  });

  it("五种事件类型都有文案", () => {
    const i18n = setup();
    for (const [type, label] of [
      ["restock", "补货"],
      ["soldout", "售罄"],
      ["price_down", "降价"],
      ["price_up", "涨价"],
      ["new_product", "新品"],
    ] as const) {
      const { getByText, unmount } = render(
        <I18nextProvider i18n={i18n}>
          <EventTag type={type} />
        </I18nextProvider>,
      );
      expect(getByText(label)).toBeInTheDocument();
      unmount();
    }
  });

  it("未知类型原样展示", () => {
    const i18n = setup();
    const { getByText } = render(
      <I18nextProvider i18n={i18n}>
        <EventTag type="mystery" />
      </I18nextProvider>,
    );
    expect(getByText("mystery")).toBeInTheDocument();
  });
});
