import { useEffect, useState } from "react";
import { App, Button, Card, Form, Select, Switch } from "antd";
import { useTranslation } from "react-i18next";
import type { ConfigView } from "../../types";
import { configUpdate } from "../../api/ipc";
import { ipcErrorMessage } from "../../i18n/errors";
import { SUPPORTED_LANGUAGES } from "../../i18n";

interface AppValues {
  autoStart: boolean;
  language: string;
  logLevel: string;
}

/** 应用设置：开机自启 / 界面语言 / 日志级别 */
export function AppSection({
  config,
  onSaved,
}: {
  config: ConfigView | null;
  onSaved: () => void;
}) {
  const { t, i18n } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm<AppValues>();
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (config) {
      form.setFieldsValue({
        autoStart: config.settings.autoStart,
        language: config.settings.language,
        logLevel: config.settings.logLevel,
      });
    }
  }, [config, form]);

  const save = async (v: AppValues) => {
    setSaving(true);
    try {
      await configUpdate({
        settings: {
          autoStart: v.autoStart,
          language: v.language,
          logLevel: v.logLevel,
        },
      });
      if (v.language !== i18n.language) {
        await i18n.changeLanguage(v.language);
      }
      message.success(t("settings.saved"));
      onSaved();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card size="small" title={t("settings.app.title")} style={{ maxWidth: 640 }}>
      <Form<AppValues> form={form} layout="vertical" onFinish={(v) => void save(v)}>
        <Form.Item name="autoStart" valuePropName="checked" label={t("settings.app.autoStart")}>
          <Switch />
        </Form.Item>
        <Form.Item name="language" label={t("settings.app.language")}>
          <Select style={{ width: 180 }} options={SUPPORTED_LANGUAGES.map((l) => ({ ...l }))} />
        </Form.Item>
        <Form.Item name="logLevel" label={t("settings.app.logLevel")}>
          <Select
            style={{ width: 180 }}
            options={["trace", "debug", "info", "warn", "error"].map((v) => ({ value: v, label: v }))}
          />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={saving}>
          {t("common.save")}
        </Button>
      </Form>
    </Card>
  );
}
