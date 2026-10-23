import { useCallback, useEffect, useState } from "react";
import { App, Button, Card, Checkbox, Form, Input, InputNumber, Popconfirm, Select, Space, Table, Tag } from "antd";
import { useTranslation } from "react-i18next";
import { ALL_EVENT_TYPES, type Rule, type RuleInput, type VendorStat } from "../types";
import { ruleCreate, ruleDelete, ruleUpdate, rulesList, vendorsList } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { EventTag } from "../components/EventTag";

interface FormValues {
  name: string;
  eventTypes: string[];
  vendorKey?: string;
  regionLike?: string;
  nameLike?: string;
  routeLike?: string;
  minPriceUsd?: number | null;
  maxPriceUsd?: number | null;
  onlyWatchlist: boolean;
  enabled: boolean;
  channels: string[];
  cooldownMinutes: number;
}

const emptyValues: FormValues = {
  name: "",
  eventTypes: ["restock"],
  vendorKey: undefined,
  regionLike: "",
  nameLike: "",
  routeLike: "",
  minPriceUsd: null,
  maxPriceUsd: null,
  onlyWatchlist: false,
  enabled: true,
  channels: ["local"],
  cooldownMinutes: 30,
};

export default function Rules() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm<FormValues>();
  const [items, setItems] = useState<Rule[]>([]);
  const [vendors, setVendors] = useState<VendorStat[]>([]);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(async () => {
    try {
      setItems(await rulesList());
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    }
  }, [message, t]);

  useEffect(() => {
    void reload();
    void vendorsList()
      .then(setVendors)
      .catch(() => undefined);
  }, [reload]);

  const toInput = (v: FormValues): RuleInput => ({
    name: v.name.trim(),
    enabled: v.enabled,
    eventTypes: v.eventTypes.join(","),
    vendorKey: v.vendorKey || null,
    regionLike: v.regionLike?.trim() || null,
    nameLike: v.nameLike?.trim() || null,
    routeLike: v.routeLike?.trim() || null,
    minPriceUsd: v.minPriceUsd ?? null,
    maxPriceUsd: v.maxPriceUsd ?? null,
    onlyWatchlist: v.onlyWatchlist,
    channel: v.channels.join(","),
    cooldownMinutes: v.cooldownMinutes,
  });

  const submit = async (values: FormValues) => {
    setSaving(true);
    try {
      const input = toInput(values);
      if (editingId == null) {
        await ruleCreate(input);
        message.success(t("rules.created"));
      } else {
        await ruleUpdate(editingId, input);
        message.success(t("rules.updated"));
      }
      form.resetFields();
      setEditingId(null);
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSaving(false);
    }
  };

  const startEdit = (rule: Rule) => {
    setEditingId(rule.id);
    form.setFieldsValue({
      name: rule.name,
      eventTypes: rule.eventTypes.split(",").map((s) => s.trim()).filter(Boolean),
      vendorKey: rule.vendorKey ?? undefined,
      regionLike: rule.regionLike ?? "",
      nameLike: rule.nameLike ?? "",
      routeLike: rule.routeLike ?? "",
      minPriceUsd: rule.minPriceUsd,
      maxPriceUsd: rule.maxPriceUsd,
      onlyWatchlist: rule.onlyWatchlist,
      enabled: rule.enabled,
      channels: rule.channel.split(",").map((s) => s.trim()).filter(Boolean),
      cooldownMinutes: rule.cooldownMinutes,
    });
  };

  const remove = async (id: number) => {
    try {
      await ruleDelete(id);
      message.success(t("rules.deleted"));
      if (editingId === id) {
        setEditingId(null);
        form.resetFields();
      }
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    }
  };

  const conditionsText = (rule: Rule) => {
    const parts: string[] = [];
    if (rule.vendorKey) {
      const v = vendors.find((x) => x.key === rule.vendorKey);
      parts.push(`${t("rules.vendor")}: ${v?.name ?? rule.vendorKey}`);
    }
    if (rule.regionLike) parts.push(`${t("rules.regionLike")}: ${rule.regionLike}`);
    if (rule.nameLike) parts.push(`${t("rules.nameLike")}: ${rule.nameLike}`);
    if (rule.routeLike) parts.push(`${t("rules.routeLike")}: ${rule.routeLike}`);
    if (rule.minPriceUsd != null) parts.push(`≥ $${rule.minPriceUsd}`);
    if (rule.maxPriceUsd != null) parts.push(`≤ $${rule.maxPriceUsd}`);
    if (rule.onlyWatchlist) parts.push(t("rules.onlyWatchlist"));
    return parts.length ? parts.join(" · ") : t("rules.noConditions");
  };

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card size="small" title={editingId == null ? t("rules.create") : t("rules.editTitle")}>
        <Form<FormValues>
          form={form}
          layout="vertical"
          initialValues={emptyValues}
          onFinish={(v) => void submit(v)}
          style={{ maxWidth: 760 }}
        >
          <Space size={12} wrap align="start">
            <Form.Item
              name="name"
              label={t("rules.name")}
              rules={[{ required: true, whitespace: true }]}
              style={{ minWidth: 220 }}
            >
              <Input maxLength={100} placeholder={t("rules.namePlaceholder")} />
            </Form.Item>
            <Form.Item
              name="eventTypes"
              label={t("rules.eventTypes")}
              rules={[{ required: true }]}
            >
              <Checkbox.Group
                options={ALL_EVENT_TYPES.map((type) => ({
                  value: type,
                  label: t(`event.${type}`),
                }))}
              />
            </Form.Item>
          </Space>
          <Space size={12} wrap align="start">
            <Form.Item name="vendorKey" label={t("rules.vendor")}>
              <Select
                style={{ width: 180 }}
                allowClear
                placeholder={t("rules.allVendors")}
                options={vendors.map((v) => ({ value: v.key, label: v.name }))}
              />
            </Form.Item>
            <Form.Item name="regionLike" label={t("rules.regionLike")}>
              <Input placeholder={t("rules.regionPlaceholder")} style={{ width: 160 }} />
            </Form.Item>
            <Form.Item name="nameLike" label={t("rules.nameLike")}>
              <Input placeholder={t("rules.nameLikePlaceholder")} style={{ width: 160 }} />
            </Form.Item>
            <Form.Item name="routeLike" label={t("rules.routeLike")}>
              <Input placeholder={t("rules.routePlaceholder")} style={{ width: 160 }} />
            </Form.Item>
          </Space>
          <Space size={12} wrap align="start">
            <Form.Item name="minPriceUsd" label={t("rules.minPrice")}>
              <InputNumber min={0} step={0.01} style={{ width: 120 }} />
            </Form.Item>
            <Form.Item name="maxPriceUsd" label={t("rules.maxPrice")}>
              <InputNumber min={0} step={0.01} style={{ width: 120 }} />
            </Form.Item>
            <Form.Item name="channels" label={t("rules.channel")} rules={[{ required: true }]}>
              <Select
                mode="multiple"
                style={{ minWidth: 260 }}
                options={[
                  { value: "local", label: t("channel.local") },
                  { value: "telegram", label: t("channel.telegram") },
                  { value: "webhook", label: t("channel.webhook") },
                  { value: "windows", label: t("channel.windows") },
                  { value: "ntfy", label: t("channel.ntfy") },
                ]}
              />
            </Form.Item>
            <Form.Item name="cooldownMinutes" label={t("rules.cooldown")}>
              <InputNumber min={0} max={1440} style={{ width: 100 }} />
            </Form.Item>
            <Form.Item name="onlyWatchlist" valuePropName="checked" label=" ">
              <Checkbox>{t("rules.onlyWatchlist")}</Checkbox>
            </Form.Item>
            <Form.Item name="enabled" valuePropName="checked" label=" ">
              <Checkbox>{t("common.enabled")}</Checkbox>
            </Form.Item>
          </Space>
          <Space>
            <Button type="primary" htmlType="submit" loading={saving}>
              {t("rules.save")}
            </Button>
            {editingId != null && (
              <Button
                onClick={() => {
                  setEditingId(null);
                  form.resetFields();
                }}
              >
                {t("rules.cancelEdit")}
              </Button>
            )}
          </Space>
        </Form>
      </Card>

      <Card size="small" title={t("rules.title")}>
        <Table<Rule>
          rowKey="id"
          size="small"
          dataSource={items}
          pagination={false}
          locale={{ emptyText: t("common.empty") }}
          columns={[
            {
              title: t("common.status"),
              dataIndex: "enabled",
              width: 80,
              render: (v: boolean) =>
                v ? <Tag color="green">{t("common.enabled")}</Tag> : <Tag>{t("common.disabled")}</Tag>,
            },
            { title: t("common.name"), dataIndex: "name", width: 180 },
            {
              title: t("rules.colEvents"),
              dataIndex: "eventTypes",
              width: 200,
              render: (v: string) => (
                <Space size={4} wrap>
                  {v
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean)
                    .map((type) => (
                      <EventTag key={type} type={type} />
                    ))}
                </Space>
              ),
            },
            {
              title: t("rules.colConditions"),
              render: (_, rule) => conditionsText(rule),
            },
            {
              title: t("rules.colChannel"),
              dataIndex: "channel",
              width: 170,
              render: (c: string) => (
                <Space size={4} wrap>
                  {c
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean)
                    .map((ch) => (
                      <Tag key={ch}>{t(`channel.${ch}`)}</Tag>
                    ))}
                </Space>
              ),
            },
            { title: t("rules.colCooldown"), dataIndex: "cooldownMinutes", width: 90 },
            {
              title: t("common.actions"),
              width: 140,
              render: (_, rule) => (
                <Space>
                  <Button size="small" onClick={() => startEdit(rule)}>
                    {t("common.edit")}
                  </Button>
                  <Popconfirm
                    title={t("rules.deleteConfirm")}
                    onConfirm={() => void remove(rule.id)}
                  >
                    <Button size="small" danger>
                      {t("common.delete")}
                    </Button>
                  </Popconfirm>
                </Space>
              ),
            },
          ]}
        />
      </Card>
    </Space>
  );
}
