import { useEffect, useState } from "react";
import { App, Button, Card, Divider, Form, Input, Space, Typography } from "antd";
import { useTranslation } from "react-i18next";
import type { ConfigInput, ConfigView, NotifyTestResult } from "../../types";
import { configUpdate, notifyTest } from "../../api/ipc";
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
  const { t, i18n } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm<NotifyValues>();
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

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

  /** 组装并提交配置（不弹消息），返回保存后的最新配置 */
  const persist = async (v: NotifyValues): Promise<ConfigView> => {
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
    return configUpdate(input);
  };

  const save = async (v: NotifyValues) => {
    setSaving(true);
    try {
      await persist(v);
      message.success(t("settings.saved"));
      onSaved();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSaving(false);
    }
  };

  /** 测试通知：先静默保存表单（测刚填的值），再对已配置通道各发一条，汇总结果 */
  const test = async () => {
    let values: NotifyValues;
    try {
      values = await form.validateFields();
    } catch {
      return; // 校验失败由表单自行标红
    }
    setTesting(true);
    try {
      const saved = await persist(values);
      onSaved();
      const groups = [
        {
          channel: "telegram",
          label: t("settings.notify.telegram"),
          ready: saved.telegramTokenSet && saved.telegramChatId.trim() !== "",
        },
        {
          channel: "webhook",
          label: t("settings.notify.webhook"),
          ready: saved.webhookUrl.trim() !== "",
        },
        {
          channel: "ntfy",
          label: t("settings.notify.ntfy"),
          ready:
            saved.ntfyTokenSet &&
            saved.ntfyServerUrl.trim() !== "" &&
            saved.ntfyTopic.trim() !== "",
        },
      ].filter((g) => g.ready);
      if (groups.length === 0) {
        message.warning(t("settings.notify.testNoneConfigured"));
        return;
      }
      const results = await Promise.all(
        groups.map(async (g) => {
          let result: NotifyTestResult;
          try {
            result = await notifyTest(g.channel);
          } catch (e) {
            result = { status: "failed", error: ipcErrorMessage(e, t) };
          }
          return { ...g, result };
        }),
      );
      const sep = i18n.language === "zh-CN" ? "、" : ", ";
      const okLabels = results.filter((r) => r.result.status === "sent").map((r) => r.label);
      if (okLabels.length > 0) {
        message.success(t("settings.notify.testSent", { channels: okLabels.join(sep) }));
      }
      for (const r of results.filter((x) => x.result.status !== "sent")) {
        message.error(
          t("settings.notify.testFailed", {
            channel: r.label,
            error: r.result.error || "-",
          }),
        );
      }
    } catch (e) {
      // 保存失败等异常
      message.error(ipcErrorMessage(e, t));
    } finally {
      setTesting(false);
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

        <Space>
          <Button type="primary" htmlType="submit" loading={saving}>
            {t("common.save")}
          </Button>
          <Button onClick={() => void test()} loading={testing}>
            {t("settings.notify.testButton")}
          </Button>
        </Space>
      </Form>
    </Card>
  );
}
