import { useEffect, useState } from "react";
import { App, Button, Card, Divider, Form, Input, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type { ConfigInput, ConfigView } from "../../types";
import { configUpdate } from "../../api/ipc";
import { ipcErrorMessage } from "../../i18n/errors";

interface NotifyValues {
  telegramChatId: string;
  telegramToken: string;
  webhookUrl: string;
  ntfyServerUrl: string;
  ntfyTopic: string;
  ntfyToken: string;
}

function GroupTitle({ children }: { children: string }) {
  return (
    <Typography.Text strong style={{ display: "block", marginBottom: 12 }}>
      {children}
    </Typography.Text>
  );
}

function tokenStatusText(set: boolean | undefined, t: (k: string) => string) {
  return set ? t("settings.notify.tokenSet") : t("settings.notify.tokenUnset");
}

/** 通知设置：Telegram / Webhook / ntfy 三组，一个保存按钮统一提交 */
export function NotifySection({
  config,
  onSaved,
}: {
  config: ConfigView | null;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm<NotifyValues>();
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (config) {
      form.setFieldsValue({
        telegramChatId: config.telegramChatId,
        telegramToken: "",
        webhookUrl: config.webhookUrl,
        ntfyServerUrl: config.ntfyServerUrl,
        ntfyTopic: config.ntfyTopic,
        ntfyToken: "",
      });
    }
  }, [config, form]);

  const save = async (v: NotifyValues) => {
    setSaving(true);
    try {
      const input: ConfigInput = {
        telegramChatId: v.telegramChatId.trim(),
        webhookUrl: v.webhookUrl.trim(),
        ntfyServerUrl: v.ntfyServerUrl.trim(),
        ntfyTopic: v.ntfyTopic.trim(),
      };
      // 只在用户输入了 token 时提交（空串=清除，留空=不改）
      if (v.telegramToken.trim() !== "") {
        input.telegramToken = v.telegramToken.trim();
      }
      if (v.ntfyToken.trim() !== "") {
        input.ntfyToken = v.ntfyToken.trim();
      }
      await configUpdate(input);
      message.success(t("settings.saved"));
      onSaved();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card size="small" title={t("settings.notify.title")} style={{ maxWidth: 640 }}>
      <Form<NotifyValues> form={form} layout="vertical" onFinish={(v) => void save(v)}>
        <GroupTitle>{t("settings.notify.telegram")}</GroupTitle>
        <Form.Item label={t("settings.notify.botToken")}>
          <span>{tokenStatusText(config?.telegramTokenSet, t)}</span>
        </Form.Item>
        <Form.Item name="telegramToken" label={t("settings.notify.botToken")}>
          <Input.Password
            placeholder={t("settings.notify.tokenPlaceholder")}
            autoComplete="new-password"
          />
        </Form.Item>
        <Form.Item name="telegramChatId" label={t("settings.notify.chatId")}>
          <Input />
        </Form.Item>

        <Divider style={{ margin: "8px 0 16px" }} />
        <GroupTitle>{t("settings.notify.webhook")}</GroupTitle>
        <Form.Item name="webhookUrl" label={t("settings.notify.webhookUrl")}>
          <Input placeholder="https://..." />
        </Form.Item>

        <Divider style={{ margin: "8px 0 16px" }} />
        <GroupTitle>{t("settings.notify.ntfy")}</GroupTitle>
        <Form.Item label={t("settings.notify.token")}>
          <span>{tokenStatusText(config?.ntfyTokenSet, t)}</span>
        </Form.Item>
        <Form.Item name="ntfyServerUrl" label={t("settings.notify.ntfyServerUrl")}>
          <Input placeholder="https://ntfy.example.com" />
        </Form.Item>
        <Form.Item name="ntfyTopic" label={t("settings.notify.ntfyTopic")}>
          <Input placeholder="vps-loading" />
        </Form.Item>
        <Form.Item name="ntfyToken" label={t("settings.notify.token")}>
          <Input.Password
            placeholder={t("settings.notify.tokenPlaceholder")}
            autoComplete="new-password"
          />
        </Form.Item>

        <Button type="primary" htmlType="submit" loading={saving}>
          {t("common.save")}
        </Button>
      </Form>
    </Card>
  );
}
