import { Tag } from "antd";
import { useTranslation } from "react-i18next";
import type { EventType } from "../types";

const EVENT_COLORS: Record<string, string> = {
  restock: "green",
  soldout: "red",
  price_down: "gold",
  price_up: "orange",
  new_product: "blue",
};

const EVENT_ICONS: Record<string, string> = {
  restock: "🟢",
  soldout: "🔴",
  price_down: "💰",
  price_up: "⬆️",
  new_product: "🆕",
};

/** 事件类型标签（补货/售罄/降价/涨价/新品），未知类型原样展示 */
export function EventTag({ type }: { type: string }) {
  const { t } = useTranslation();
  const label = (Object.keys(EVENT_COLORS) as EventType[]).includes(type as EventType)
    ? t(`event.${type}`)
    : type;
  return (
    <Tag color={EVENT_COLORS[type]} icon={<span>{EVENT_ICONS[type] ?? "🔔"}</span>}>
      {label}
    </Tag>
  );
}
