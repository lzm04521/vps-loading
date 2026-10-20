import { useCallback, useEffect, useState } from "react";
import { App, Button, Card, Checkbox, Descriptions, Form, Input, InputNumber, Select, Space, Switch } from "antd";
import { FolderOpenOutlined } from "@ant-design/icons";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { ConfigView, LegacyImportReport } from "../types";
import { appOpenLogDir, configGet, configUpdate, legacyImport } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { SUPPORTED_LANGUAGES } from "../i18n";

interface GeneralValues {
  sourceUrl: string;
  pollIntervalSeconds: number;
}

interface NotifyValues {
  telegramChatId: string;
  telegramToken: string;
  webhookUrl: string;
}

interface AppValues {
  autoStart: boolean;
  language: string;
  logLevel: string;
}

export default function Settings() {
  const { t, i18n } = useTranslation();
  const { message } = App.useApp();
  const [config, setConfig] = useState<ConfigView | null>(null);
  const [general] = Form.useForm<GeneralValues>();
  const [notify] = Form.useForm<NotifyValues>();
  const [appForm] = Form.useForm<AppValues>();
  const [savingGeneral, setSavingGeneral] = useState(false);
  const [savingNotify, setSavingNotify] = useState(false);
  const [savingApp, setSavingApp] = useState(false);
  const [legacyDir, setLegacyDir] = useState<string | null>(null);
  const [legacyOverwrite, setLegacyOverwrite] = useState(false);
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<LegacyImportReport | null>(null);

  const reload = useCallback(async () => {
    try {
      const c = await configGet();
      setConfig(c);
      general.setFieldsValue({
        sourceUrl: c.sourceUrl,
        pollIntervalSeconds: c.pollIntervalSeconds,
      });
      notify.setFieldsValue({
        telegramChatId: c.telegramChatId,
        telegramToken: "",
        webhookUrl: c.webhookUrl,
      });
      appForm.setFieldsValue({
        autoStart: c.settings.autoStart,
        language: c.settings.language,
        logLevel: c.settings.logLevel,
      });
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    }
  }, [general, notify, appForm, message, t]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const saveGeneral = async (v: GeneralValues) => {
    setSavingGeneral(true);
    try {
      await configUpdate({
        sourceUrl: v.sourceUrl.trim(),
        pollIntervalSeconds: v.pollIntervalSeconds,
      });
      message.success(t("settings.saved"));
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSavingGeneral(false);
    }
  };

  const saveNotify = async (v: NotifyValues) => {
    setSavingNotify(true);
    try {
      const input: Parameters<typeof configUpdate>[0] = {
        telegramChatId: v.telegramChatId.trim(),
        webhookUrl: v.webhookUrl.trim(),
      };
      // 只在用户输入了 token 时提交（空串=清除，留空=不改）
      if (v.telegramToken.trim() !== "") {
        input.telegramToken = v.telegramToken.trim();
      }
      await configUpdate(input);
      message.success(t("settings.saved"));
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSavingNotify(false);
    }
  };

  const saveApp = async (v: AppValues) => {
    setSavingApp(true);
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
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setSavingApp(false);
    }
  };

  const pickLegacyDir = async () => {
    try {
      const dir = await open({ directory: true, multiple: false });
      if (typeof dir === "string") {
        setLegacyDir(dir);
        setReport(null);
      }
    } catch {
      // 用户取消或对话框不可用
    }
  };

  const runImport = async () => {
    if (!legacyDir) return;
    setImporting(true);
    try {
      const r = await legacyImport(legacyDir, legacyOverwrite);
      setReport(r);
      message.success(t("settings.legacy.imported"));
      await reload();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card size="small" title={t("settings.collect.title")} style={{ maxWidth: 640 }}>
        <Form<GeneralValues> form={general} layout="vertical" onFinish={(v) => void saveGeneral(v)}>
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
          <Button type="primary" htmlType="submit" loading={savingGeneral}>
            {t("common.save")}
          </Button>
        </Form>
      </Card>

      <Card size="small" title={t("settings.notify.title")} style={{ maxWidth: 640 }}>
        <Form<NotifyValues> form={notify} layout="vertical" onFinish={(v) => void saveNotify(v)}>
          <Form.Item label={`Telegram ${t("settings.notify.botToken")}`}>
            <Space>
              <span>
                {config?.telegramTokenSet
                  ? t("settings.notify.tokenSet")
                  : t("settings.notify.tokenUnset")}
              </span>
            </Space>
          </Form.Item>
          <Form.Item name="telegramToken" label={t("settings.notify.botToken")}>
            <Input.Password placeholder={t("settings.notify.tokenPlaceholder")} autoComplete="new-password" />
          </Form.Item>
          <Form.Item name="telegramChatId" label={t("settings.notify.chatId")}>
            <Input />
          </Form.Item>
          <Form.Item name="webhookUrl" label={t("settings.notify.webhookUrl")}>
            <Input placeholder="https://..." />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={savingNotify}>
            {t("common.save")}
          </Button>
        </Form>
      </Card>

      <Card size="small" title={t("settings.app.title")} style={{ maxWidth: 640 }}>
        <Form<AppValues> form={appForm} layout="vertical" onFinish={(v) => void saveApp(v)}>
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
          <Button type="primary" htmlType="submit" loading={savingApp}>
            {t("common.save")}
          </Button>
        </Form>
      </Card>

      <Card size="small" title={t("settings.paths.title")} style={{ maxWidth: 640 }}>
        <Descriptions size="small" column={1} bordered>
          <Descriptions.Item label={t("settings.paths.configPath")}>
            {config?.configPath ?? "-"}
          </Descriptions.Item>
          <Descriptions.Item label={t("settings.paths.dbPath")}>
            {config?.dbPath ?? "-"}
          </Descriptions.Item>
        </Descriptions>
        <Button style={{ marginTop: 12 }} icon={<FolderOpenOutlined />} onClick={() => void appOpenLogDir().catch((e) => message.error(ipcErrorMessage(e, t)))}>
          {t("settings.paths.openLogDir")}
        </Button>
      </Card>

      <Card size="small" title={t("settings.legacy.title")} style={{ maxWidth: 640 }}>
        <span style={{ display: "block", color: "#888", marginBottom: 8 }}>
          {t("settings.legacy.desc")}
        </span>
        <Space direction="vertical" size={8}>
          <Space wrap>
            <Button icon={<FolderOpenOutlined />} onClick={() => void pickLegacyDir()}>
              {t("settings.legacy.pick")}
            </Button>
            <span>{legacyDir ?? t("settings.legacy.dirPlaceholder")}</span>
          </Space>
          <Checkbox
            checked={legacyOverwrite}
            onChange={(e) => setLegacyOverwrite(e.target.checked)}
          >
            {t("settings.legacy.overwrite")}
          </Checkbox>
          <Button
            type="primary"
            disabled={!legacyDir}
            loading={importing}
            onClick={() => void runImport()}
          >
            {t("settings.legacy.import")}
          </Button>
          {report && (
            <ul style={{ margin: 0, paddingLeft: 20, color: "#389e0d" }}>
              {report.detail.map((line, i) => (
                <li key={i}>{line}</li>
              ))}
            </ul>
          )}
        </Space>
      </Card>
    </Space>
  );
}
