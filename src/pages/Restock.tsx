import { useCallback, useEffect, useState } from "react";
import { App, Card, List, Select, Space, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type { EventView, HistoryView } from "../types";
import { eventsList, sourceHistoryList } from "../api/ipc";
import { openExternal } from "../api/external";
import { ipcErrorMessage } from "../i18n/errors";
import { EventTag } from "../components/EventTag";
import { formatDateTime, formatRelative } from "../time";
import { priceWithUnit } from "../price";

export default function Restock() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [events, setEvents] = useState<EventView[]>([]);
  const [history, setHistory] = useState<HistoryView[]>([]);
  const [typeFilter, setTypeFilter] = useState<string | undefined>();

  const reload = useCallback(async () => {
    try {
      const [e, h] = await Promise.all([eventsList(100, typeFilter ?? null), sourceHistoryList(30)]);
      setEvents(e);
      setHistory(h);
    } catch (err) {
      message.error(ipcErrorMessage(err, t));
    }
  }, [typeFilter, message, t]);

  useEffect(() => {
    void reload();
    const timer = window.setInterval(() => void reload(), 60_000);
    return () => window.clearInterval(timer);
  }, [reload]);

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card
        size="small"
        title={t("restock.localTitle")}
        extra={
          <Select
            style={{ width: 140 }}
            placeholder={t("restock.filterType")}
            allowClear
            value={typeFilter}
            onChange={setTypeFilter}
            options={[
              { value: "restock", label: t("event.restock") },
              { value: "soldout", label: t("event.soldout") },
              { value: "price_down", label: t("event.price_down") },
              { value: "price_up", label: t("event.price_up") },
              { value: "new_product", label: t("event.new_product") },
            ]}
          />
        }
      >
        <Typography.Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
          {t("restock.localDesc")}
        </Typography.Text>
        <List
          size="small"
          dataSource={events}
          locale={{ emptyText: t("common.empty") }}
          renderItem={(e) => (
            <List.Item>
              <Space size={8} wrap>
                <EventTag type={e.type} />
                <span>
                  <Typography.Text strong>
                    {e.vendorName} · {e.productName}
                  </Typography.Text>
                </span>
                {e.regionCn && <Tag bordered={false}>{e.regionCn}</Tag>}
                {e.currentPrice && <Tag bordered={false}>{priceWithUnit(e.currentPrice, e.unit)}</Tag>}
              </Space>
              <Space size={8}>
                <Typography.Text type="secondary">{formatRelative(e.createdAt)}</Typography.Text>
                {e.buyUrl && (
                  <Typography.Link
                    onClick={() =>
                      void openExternal(e.buyUrl).catch((err) => message.error(String(err)))
                    }
                  >
                    {t("products.buy")}
                  </Typography.Link>
                )}
              </Space>
            </List.Item>
          )}
        />
      </Card>

      <Card size="small" title={t("restock.sourceTitle")}>
        <Typography.Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
          {t("restock.sourceDesc")}
        </Typography.Text>
        <List
          size="small"
          dataSource={history}
          locale={{ emptyText: t("common.empty") }}
          renderItem={(h) => (
            <List.Item>
              <Space size={8} wrap>
                <Tag bordered={false}>{h.timeText}</Tag>
                <Typography.Text strong>{h.name}</Typography.Text>
                {h.price && <Tag bordered={false}>{h.price}</Tag>}
              </Space>
              {h.buyUrl ? (
                <Typography.Link
                  onClick={() =>
                    void openExternal(h.buyUrl).catch((e) => message.error(String(e)))
                  }
                >
                  {t("products.buy")}
                </Typography.Link>
              ) : (
                <Typography.Text type="secondary">{formatDateTime(h.occurredAt)}</Typography.Text>
              )}
            </List.Item>
          )}
        />
      </Card>
    </Space>
  );
}
