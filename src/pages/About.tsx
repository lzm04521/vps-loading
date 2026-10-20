import { Card, Descriptions, Space, Tag, Typography } from "antd";
import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

export default function About() {
  const { t } = useTranslation();
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
              href="https://github.com/lzm04521/vps-loading"
              target="_blank"
              rel="noreferrer"
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

      <Card size="small" title={t("about.updateTitle")} style={{ maxWidth: 640 }}>
        <Space direction="vertical" size={4}>
          <Typography.Paragraph style={{ marginBottom: 0 }}>
            {t("about.updateDesc")}
          </Typography.Paragraph>
          <Typography.Text type="secondary">{t("about.dataNote")}</Typography.Text>
        </Space>
      </Card>
    </Space>
  );
}
