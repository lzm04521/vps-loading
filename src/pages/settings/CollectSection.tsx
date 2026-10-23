import { useEffect, useState } from "react";
import { App, Button, Card, Form, Input, InputNumber } from "antd";
import { useTranslation } from "react-i18next";
import type { ConfigView } from "../../types";
import { configUpdate } from "../../api/ipc";
import { ipcErrorMessage } from "../../i18n/errors";

interface GeneralValues {
  sourceUrl: string;
  pollIntervalSeconds: number;
}

/** 采集设置：数据源地址 + 采集间隔 */
export function CollectSection({
  config,
  onSaved,
}: {
  config: ConfigView | null;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm<GeneralValues>();
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (config) {
      form.setFieldsValue({
        sourceUrl: config.sourceUrl,
        pollIntervalSeconds: config.pollIntervalSeconds,
      });
    }
  }, [config, form]);

  const save = async (v: GeneralValues) => {
    setSaving(true);
    try {
      await configUpdate({
        sourceUrl: v.sourceUrl.trim(),
        pollIntervalSeconds: v.pollIntervalSeconds,
      });
      message.success(t("settings.saved"));
      onSaved();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card size="small" title={t("settings.collect.title")} style={{ maxWidth: 640 }}>
      <Form<GeneralValues> form={form} layout="vertical" onFinish={(v) => void save(v)}>
        <Form.Item
          name="sourceUrl"
          label={t("settings.collect.sourceUrl")}
          rules={[
            { required: true },
            {
              validator: (_, value: string) =>
                value.startsWith("http://") || value.startsWith("https://")
                  ? Promise.resolve()
                  : Promise.reject(new Error("http(s)://")),
            },
          ]}
        >
          <Input placeholder="https://kyoa.de/data.js" />
        </Form.Item>
        <Form.Item
          name="pollIntervalSeconds"
          label={t("settings.collect.interval")}
          rules={[{ required: true }]}
        >
          <InputNumber min={60} max={86400} style={{ width: 160 }} />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={saving}>
          {t("common.save")}
        </Button>
      </Form>
    </Card>
  );
}
