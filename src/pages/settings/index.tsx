import { useCallback, useEffect, useState } from "react";
import { App, Grid, Menu } from "antd";
import { useTranslation } from "react-i18next";
import type { ConfigView } from "../../types";
import { configGet } from "../../api/ipc";
import { ipcErrorMessage } from "../../i18n/errors";
import { CollectSection } from "./CollectSection";
import { NotifySection } from "./NotifySection";
import { AppSection } from "./AppSection";
import { DataSection } from "./DataSection";

type SectionKey = "collect" | "notify" | "app" | "data";

/** 设置页：左侧分块菜单 + 右侧内容（小屏退化为顶部横排菜单） */
export default function Settings() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [config, setConfig] = useState<ConfigView | null>(null);
  const [active, setActive] = useState<SectionKey>("collect");
  const screens = Grid.useBreakpoint();
  const wide = screens.md ?? false;

  const reload = useCallback(async () => {
    try {
      setConfig(await configGet());
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    }
  }, [message, t]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const menu = (
    <Menu
      mode={wide ? "vertical" : "horizontal"}
      selectedKeys={[active]}
      items={[
        { key: "collect", label: t("settings.sections.collect") },
        { key: "notify", label: t("settings.sections.notify") },
        { key: "app", label: t("settings.sections.app") },
        { key: "data", label: t("settings.sections.data") },
      ]}
      onClick={({ key }) => setActive(key as SectionKey)}
    />
  );

  return (
    <div
      style={{
        display: "flex",
        flexDirection: wide ? "row" : "column",
        gap: 16,
        alignItems: wide ? "stretch" : "stretch",
      }}
    >
      <div
        style={
          wide
            ? { width: 176, flexShrink: 0, position: "sticky", top: 24, alignSelf: "flex-start" }
            : undefined
        }
      >
        {menu}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        {active === "collect" && <CollectSection config={config} onSaved={() => void reload()} />}
        {active === "notify" && <NotifySection config={config} onSaved={() => void reload()} />}
        {active === "app" && <AppSection config={config} onSaved={() => void reload()} />}
        {active === "data" && <DataSection config={config} onSaved={() => void reload()} />}
      </div>
    </div>
  );
}
