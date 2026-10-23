import { useState } from "react";
import { App, Button, Card, Checkbox, Descriptions, Divider, Space, Typography } from "antd";
import { FolderOpenOutlined } from "@ant-design/icons";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { ConfigView, LegacyImportReport } from "../../types";
import { appOpenLogDir, legacyImport } from "../../api/ipc";
import { ipcErrorMessage } from "../../i18n/errors";

/** 数据管理：配置/数据库位置 + 日志目录 + 旧版数据导入 */
export function DataSection({
  config,
  onSaved,
}: {
  config: ConfigView | null;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [legacyDir, setLegacyDir] = useState<string | null>(null);
  const [legacyOverwrite, setLegacyOverwrite] = useState(false);
  const [importing, setImporting] = useState(false);
  const [report, setReport] = useState<LegacyImportReport | null>(null);

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
      onSaved();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Card size="small" title={t("settings.paths.title")} style={{ maxWidth: 640 }}>
      <Descriptions size="small" column={1} bordered>
        <Descriptions.Item label={t("settings.paths.configPath")}>
          {config?.configPath ?? "-"}
        </Descriptions.Item>
        <Descriptions.Item label={t("settings.paths.dbPath")}>
          {config?.dbPath ?? "-"}
        </Descriptions.Item>
      </Descriptions>
      <Button
        style={{ marginTop: 12 }}
        icon={<FolderOpenOutlined />}
        onClick={() => void appOpenLogDir().catch((e) => message.error(ipcErrorMessage(e, t)))}
      >
        {t("settings.paths.openLogDir")}
      </Button>

      <Divider style={{ margin: "16px 0" }} />
      <Typography.Text strong style={{ display: "block", marginBottom: 12 }}>
        {t("settings.legacy.title")}
      </Typography.Text>
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
  );
}
