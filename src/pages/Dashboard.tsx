import { useCallback, useEffect, useState } from "react";
import { App, Button, Card, Col, Descriptions, Row, Space, Statistic, Table, Tag, Tooltip } from "antd";
import { ReloadOutlined, SyncOutlined, ThunderboltOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { EventView, SummaryView } from "../types";
import { collectManual, eventsList, onStatusEvent, summaryGet } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { EventTag } from "../components/EventTag";
import { formatDateTime, formatRelative } from "../time";
import { priceWithUnit } from "../price";

export default function Dashboard() {
  const { t } = useTranslation();
  const { message, notification } = App.useApp();
  const [summary, setSummary] = useState<SummaryView | null>(null);
  const [events, setEvents] = useState<EventView[]>([]);
  const [collecting, setCollecting] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [s, e] = await Promise.all([summaryGet(), eventsList(10, null)]);
      setSummary(s);
      setEvents(e);
    } catch (err) {
      message.error(ipcErrorMessage(err, t));
    }
  }, [message, t]);

  useEffect(() => {
    void reload();
    const timer = window.setInterval(() => void reload(), 60_000);
    // 引擎状态事件：采集开始/结束/失败时刷新，并轻提示；
    // 非 Tauri 环境（测试）下 listen 失败，忽略
    const unlisten = onStatusEvent((ev) => {
      if (ev.state === "done" || ev.state === "error") {
        void reload();
        if (ev.state === "error") {
          notification.warning({ message: t("dashboard.collector.title"), description: ev.message });
        }
      }
      if (ev.state === "skipped") setCollecting(false);
    });
    return () => {
      window.clearInterval(timer);
      void unlisten
        .then((fn) => fn())
        .catch(() => undefined);
    };
  }, [reload, notification, t]);

  const runCollect = async (force: boolean) => {
    setCollecting(true);
    try {
      const result = await collectManual(force);
      if (result.skipped) {
        message.warning(t("dashboard.collector.busy"));
      } else if (!result.ok && result.error) {
        message.error(result.error);
      }
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setCollecting(false);
    }
  };

  const c = summary?.counts;
  const busy = summary?.collectorBusy ?? collecting;

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Row gutter={[16, 16]}>
        <Col xs={12} md={4}>
          <Card size="small">
            <Statistic title={t("dashboard.stats.total")} value={c?.total ?? 0} />
          </Card>
        </Col>
        <Col xs={12} md={4}>
          <Card size="small">
            <Statistic
              title={t("dashboard.stats.ok")}
              value={c?.ok ?? 0}
              valueStyle={{ color: "#389e0d" }}
            />
          </Card>
        </Col>
        <Col xs={12} md={4}>
          <Card size="small">
            <Statistic
              title={t("dashboard.stats.oos")}
              value={c?.oos ?? 0}
              valueStyle={{ color: "#cf1322" }}
            />
          </Card>
        </Col>
        <Col xs={12} md={4}>
          <Card size="small">
            <Statistic title={t("dashboard.stats.watched")} value={c?.watched ?? 0} />
          </Card>
        </Col>
        <Col xs={12} md={4}>
          <Card size="small">
            <Statistic title={t("dashboard.stats.events24h")} value={c?.events24h ?? 0} />
          </Card>
        </Col>
        <Col xs={12} md={4}>
          <Card size="small">
            <Statistic title={t("dashboard.stats.alerts24h")} value={c?.alerts24h ?? 0} />
          </Card>
        </Col>
      </Row>

      <Card
        size="small"
        title={t("dashboard.collector.title")}
        extra={
          <Space>
            <Tag color={busy ? "processing" : "default"}>
              {busy ? t("dashboard.collector.busy") : t("dashboard.collector.idle")}
            </Tag>
            <Button
              icon={<SyncOutlined spin={busy} />}
              loading={collecting}
              onClick={() => void runCollect(false)}
            >
              {t("dashboard.collector.collectNow")}
            </Button>
            <Tooltip title={t("dashboard.collector.forceHint")}>
              <Button icon={<ThunderboltOutlined />} onClick={() => void runCollect(true)}>
                {t("dashboard.collector.forceCollect")}
              </Button>
            </Tooltip>
            <Button icon={<ReloadOutlined />} onClick={() => void reload()} />
          </Space>
        }
      >
        <Descriptions size="small" column={{ xs: 1, md: 2 }} bordered>
          <Descriptions.Item label={t("dashboard.collector.sourceUrl")}>
            {summary?.sourceUrl ?? "-"}
          </Descriptions.Item>
          <Descriptions.Item label={t("dashboard.collector.interval")}>
            {summary ? t("dashboard.collector.intervalValue", { seconds: summary.pollIntervalSeconds }) : "-"}
          </Descriptions.Item>
          <Descriptions.Item label={t("dashboard.collector.lastChecked")}>
            {formatDateTime(summary?.lastChecked)}
          </Descriptions.Item>
          <Descriptions.Item label={t("dashboard.collector.lastSuccess")}>
            {formatDateTime(summary?.lastSuccess)}
          </Descriptions.Item>
          <Descriptions.Item label={t("dashboard.collector.lastDataChange")}>
            {formatDateTime(summary?.lastDataChange)}
          </Descriptions.Item>
          <Descriptions.Item label={t("dashboard.channels.title")}>
            <Space>
              <Tag color={summary?.telegramConfigured ? "green" : "default"}>
                {summary?.telegramConfigured
                  ? t("dashboard.channels.telegramReady")
                  : t("dashboard.channels.telegramNotReady")}
              </Tag>
              <Tag color={summary?.webhookConfigured ? "green" : "default"}>
                {summary?.webhookConfigured
                  ? t("dashboard.channels.webhookReady")
                  : t("dashboard.channels.webhookNotReady")}
              </Tag>
            </Space>
          </Descriptions.Item>
          {summary?.lastError ? (
            <Descriptions.Item label={t("dashboard.collector.lastError")} span={2}>
              <span style={{ color: "#cf1322" }}>{summary.lastError}</span>
            </Descriptions.Item>
          ) : null}
        </Descriptions>
      </Card>

      <Card size="small" title={t("dashboard.recentEvents")}>
        <Table<EventView>
          rowKey="id"
          size="small"
          dataSource={events}
          pagination={false}
          locale={{ emptyText: t("common.empty") }}
          columns={[
            {
              title: t("common.status"),
              dataIndex: "type",
              width: 110,
              render: (type: string) => <EventTag type={type} />,
            },
            {
              title: t("common.product"),
              render: (_, e) => `${e.vendorName} · ${e.productName}`,
            },
            {
              title: t("common.region"),
              dataIndex: "regionCn",
              width: 110,
              responsive: ["md"],
            },
            {
              title: t("common.price"),
              width: 160,
              render: (_, e) => {
                // 空价回退美元折算价，折算参考价不带周期
                const priceText = e.currentPrice
                  ? priceWithUnit(e.currentPrice, e.unit)
                  : e.currentPriceUsd != null
                    ? `$${e.currentPriceUsd.toFixed(2)}`
                    : "-";
                const changed =
                  e.oldPriceUsd != null &&
                  e.newPriceUsd != null &&
                  Math.abs(e.oldPriceUsd - e.newPriceUsd) >= 0.005;
                return changed ? `${e.oldPriceUsd} → ${e.newPriceUsd}（${priceText}）` : priceText;
              },
            },
            {
              title: t("common.time"),
              dataIndex: "createdAt",
              width: 150,
              render: (v: string) => (
                <Tooltip title={formatDateTime(v)}>
                  <span>{formatRelative(v)}</span>
                </Tooltip>
              ),
            },
          ]}
        />
      </Card>
    </Space>
  );
}
