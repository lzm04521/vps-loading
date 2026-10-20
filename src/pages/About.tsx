import { App, Button, Card, Descriptions, Progress, Space, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openExternal } from "../api/external";
import { useUpdater } from "../hooks/useUpdater";

const REPO_URL = "https://github.com/lzm04521/vps-loading";

function formatMb(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** "升级方式"卡片：检查更新 → 下载安装（进度）→ 重启生效 */
function UpdatePanel() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const { state, version, error, progress, check, install, restart } = useUpdater();

  const percent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;

  return (
    <Card size="small" title={t("about.updateTitle")} style={{ maxWidth: 640 }}>
      <Space direction="vertical" size={8} style={{ width: "100%" }}>
        <Typography.Paragraph style={{ marginBottom: 0 }}>
          {t("about.updateDesc")}
        </Typography.Paragraph>
        <Typography.Text type="secondary">{t("about.dataNote")}</Typography.Text>

        {state === "available" && version && (
          <Typography.Text strong>{t("about.found", { version })}</Typography.Text>
        )}
        {state === "uptodate" && (
          <Typography.Text type="success">{t("about.uptodate")}</Typography.Text>
        )}
        {state === "ready" && (
          <Typography.Text type="success">{t("about.readyToRestart")}</Typography.Text>
        )}
        {state === "error" && error && (
          <Typography.Text type="danger">
            {t("about.updateFailed", { detail: error })}
          </Typography.Text>
        )}
        {state === "downloading" && (
          <Space direction="vertical" size={2} style={{ width: "100%" }}>
            <Typography.Text type="secondary">
              {t("about.downloading")}
              {progress &&
                ` ${formatMb(progress.downloaded)}${
                  progress.total ? ` / ${formatMb(progress.total)}` : ""
                }`}
            </Typography.Text>
            {percent != null && (
              <Progress
                percent={percent}
                size="small"
                style={{ maxWidth: 320, marginBottom: 0 }}
              />
            )}
          </Space>
        )}

        <Space wrap>
          {(state === "idle" || state === "checking" || state === "uptodate" || state === "error") && (
            <Button
              size="small"
              loading={state === "checking"}
              onClick={() => void check()}
            >
              {t("about.checkNow")}
            </Button>
          )}
          {state === "available" && (
            <Button size="small" type="primary" onClick={() => void install()}>
              {t("about.downloadInstall")}
            </Button>
          )}
          {state === "ready" && (
            <Button
              size="small"
              type="primary"
              onClick={() => void restart().catch((e) => message.error(String(e)))}
            >
              {t("about.restartNow")}
            </Button>
          )}
        </Space>
      </Space>
    </Card>
  );
}

export default function About() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [version, setVersion] = useState<string>("");

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Card size="small" title={t("about.title")} style={{ maxWidth: 640 }}>
        <Descriptions size="small" column={1} bordered>
          <Descriptions.Item label={t("app.title")}>
            <Space>
              <span>VPS Monitor</span>
              {version && <Tag color="blue">v{version}</Tag>}
            </Space>
          </Descriptions.Item>
          <Descriptions.Item label={t("about.sourceRepo")}>
            <Typography.Link
              onClick={() => void openExternal(REPO_URL).catch((e) => message.error(String(e)))}
            >
              github.com/lzm04521/vps-loading
            </Typography.Link>
          </Descriptions.Item>
        </Descriptions>
      </Card>

      <Card size="small" title={t("about.techTitle")} style={{ maxWidth: 640 }}>
        <Typography.Paragraph style={{ marginBottom: 0 }}>
          {t("about.techDesc")}
        </Typography.Paragraph>
      </Card>

      <UpdatePanel />
    </Space>
  );
}
