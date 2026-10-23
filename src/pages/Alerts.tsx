import { useCallback, useEffect, useState } from "react";
import { App, Card, Table, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type { AlertView, Channel } from "../types";
import { alertsList } from "../api/ipc";
import { openExternal } from "../api/external";
import { ipcErrorMessage } from "../i18n/errors";
import { formatDateTime } from "../time";

const CHANNEL_COLORS: Record<string, string> = {
  local: "default",
  telegram: "blue",
  webhook: "purple",
  windows: "green",
  ntfy: "geekblue",
};

function statusTag(status: string, t: (k: string) => string) {
  if (status === "sent") return <Tag color="green">{t("alerts.statusSent")}</Tag>;
  if (status === "local") return <Tag>{t("alerts.statusLocal")}</Tag>;
  return <Tag color="red">{t("alerts.statusFailed")}</Tag>;
}

export default function Alerts() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [items, setItems] = useState<AlertView[]>([]);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await alertsList(200));
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setLoading(false);
    }
  }, [message, t]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return (
    <Card size="small" title={t("alerts.title")}>
      <Table<AlertView>
        rowKey="id"
        size="small"
        loading={loading}
        dataSource={items}
        pagination={{ pageSize: 20, showSizeChanger: false }}
        locale={{ emptyText: t("common.empty") }}
        scroll={{ x: 900 }}
        columns={[
          {
            title: t("common.time"),
            dataIndex: "sentAt",
            width: 170,
            render: (v: string) => formatDateTime(v),
          },
          { title: t("alerts.rule"), dataIndex: "ruleName", width: 160, responsive: ["md"] },
          {
            title: t("common.product"),
            render: (_, a) => `${a.vendorName} · ${a.productName}`,
          },
          { title: t("common.region"), dataIndex: "regionCn", width: 100, responsive: ["lg"] },
          {
            title: t("alerts.channel"),
            dataIndex: "channel",
            width: 110,
            render: (c: Channel) => (
              <Tag color={CHANNEL_COLORS[c] ?? "default"}>{t(`channel.${c}`)}</Tag>
            ),
          },
          {
            title: t("common.status"),
            dataIndex: "status",
            width: 90,
            render: (s: string) => statusTag(s, t),
          },
          {
            title: t("polls.error"),
            dataIndex: "error",
            ellipsis: true,
            responsive: ["lg"],
          },
          {
            title: t("common.actions"),
            width: 70,
            fixed: "right",
            render: (_, a) =>
              a.buyUrl ? (
                <Typography.Link
                  onClick={() => void openExternal(a.buyUrl).catch((e) => message.error(String(e)))}
                >
                  {t("products.buy")}
                </Typography.Link>
              ) : (
                "-"
              ),
          },
        ]}
      />
    </Card>
  );
}
