import { useCallback, useEffect, useState } from "react";
import { App, Card, Table, Tag } from "antd";
import { useTranslation } from "react-i18next";
import type { PollView } from "../types";
import { pollsList } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { formatDateTime } from "../time";

export default function Polls() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [items, setItems] = useState<PollView[]>([]);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await pollsList(200));
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setLoading(false);
    }
  }, [message, t]);

  useEffect(() => {
    void reload();
    const timer = window.setInterval(() => void reload(), 60_000);
    return () => window.clearInterval(timer);
  }, [reload]);

  return (
    <Card size="small" title={t("polls.title")}>
      <Table<PollView>
        rowKey="id"
        size="small"
        loading={loading}
        dataSource={items}
        pagination={{ pageSize: 20, showSizeChanger: false }}
        locale={{ emptyText: t("common.empty") }}
        columns={[
          {
            title: t("common.time"),
            dataIndex: "fetchedAt",
            width: 170,
            render: (v: string) => formatDateTime(v),
          },
          {
            title: t("polls.http"),
            dataIndex: "httpStatus",
            width: 80,
            render: (v: number | null) =>
              v == null ? <Tag>-</Tag> : v === 304 || v === 200 ? <Tag color="green">{v}</Tag> : <Tag color="red">{v}</Tag>,
          },
          {
            title: t("polls.changed"),
            dataIndex: "changed",
            width: 90,
            render: (v: boolean) =>
              v ? <Tag color="blue">{t("polls.changedYes")}</Tag> : <Tag>{t("polls.changedNo")}</Tag>,
          },
          { title: t("polls.total"), dataIndex: "total", width: 80 },
          { title: t("polls.ok"), dataIndex: "ok", width: 80 },
          { title: t("polls.oos"), dataIndex: "oos", width: 80 },
          {
            title: t("polls.duration"),
            dataIndex: "durationMs",
            width: 100,
            render: (v: number | null) =>
              v == null ? "-" : t("polls.durationValue", { ms: v }),
          },
          {
            title: t("polls.error"),
            dataIndex: "error",
            ellipsis: true,
            render: (v: string | null) =>
              v ? <span style={{ color: "#cf1322" }}>{v}</span> : "-",
          },
        ]}
      />
    </Card>
  );
}
