import { HashRouter, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { App as AntApp, Button, ConfigProvider, Layout, Menu } from "antd";
import zhCN from "antd/locale/zh_CN";
import enUS from "antd/locale/en_US";
import {
  BellOutlined,
  DashboardOutlined,
  DatabaseOutlined,
  FileTextOutlined,
  HistoryOutlined,
  InfoCircleOutlined,
  SettingOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useEffect, useRef } from "react";
import About from "./pages/About";
import Alerts from "./pages/Alerts";
import Dashboard from "./pages/Dashboard";
import Polls from "./pages/Polls";
import Products from "./pages/Products";
import Restock from "./pages/Restock";
import Rules from "./pages/Rules";
import Settings from "./pages/Settings";

const { Sider, Header, Content } = Layout;

const menuItems = [
  { key: "/", icon: <DashboardOutlined />, ns: "menu.dashboard" },
  { key: "/products", icon: <DatabaseOutlined />, ns: "menu.products" },
  { key: "/restock", icon: <HistoryOutlined />, ns: "menu.restock" },
  { key: "/alerts", icon: <BellOutlined />, ns: "menu.alerts" },
  { key: "/rules", icon: <ThunderboltOutlined />, ns: "menu.rules" },
  { key: "/polls", icon: <FileTextOutlined />, ns: "menu.polls" },
  { key: "/settings", icon: <SettingOutlined />, ns: "menu.settings" },
  { key: "/about", icon: <InfoCircleOutlined />, ns: "menu.about" },
];

/** 启动时静默检查一次新版本：发现新版本弹通知，引导到"关于"页升级 */
function StartupUpdateCheck() {
  const { t } = useTranslation();
  const { notification } = AntApp.useApp();
  const navigate = useNavigate();
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void (async () => {
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const update = await check();
        if (!update) return;
        notification.info({
          message: t("about.updateAvailableTitle"),
          description: t("about.updateAvailableDesc", { version: update.version }),
          placement: "bottomRight",
          btn: (
            <Button
              size="small"
              type="primary"
              onClick={() => {
                notification.destroy();
                navigate("/about");
              }}
            >
              {t("about.goToUpdate")}
            </Button>
          ),
        });
      } catch (e) {
        // 启动静默检查失败不打扰用户，仅记录控制台
        console.warn("startup update check failed:", e);
      }
    })();
  }, [notification, navigate, t]);

  return null;
}

function AppLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <StartupUpdateCheck />
      <Sider
        theme="dark"
        width={200}
        // 长页面滚动时菜单钉在左侧；窗口过矮时菜单内部可滚
        style={{ position: "sticky", top: 0, height: "100vh", overflowY: "auto" }}
      >
        <div
          style={{
            color: "#fff",
            padding: 16,
            fontWeight: 600,
            fontSize: 16,
          }}
        >
          {t("app.title")}
        </div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[location.pathname]}
          items={menuItems.map(({ key, icon, ns }) => ({
            key,
            icon,
            label: t(ns),
          }))}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header style={{ background: "#fff", padding: "0 24px" }} />
        <Content style={{ padding: 24 }}>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/products" element={<Products />} />
            <Route path="/restock" element={<Restock />} />
            <Route path="/alerts" element={<Alerts />} />
            <Route path="/rules" element={<Rules />} />
            <Route path="/polls" element={<Polls />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/about" element={<About />} />
          </Routes>
        </Content>
      </Layout>
    </Layout>
  );
}

export default function App() {
  const { i18n } = useTranslation();
  const antdLocale = i18n.language.startsWith("zh") ? zhCN : enUS;
  return (
    <ConfigProvider locale={antdLocale}>
      <AntApp>
        <HashRouter>
          <AppLayout />
        </HashRouter>
      </AntApp>
    </ConfigProvider>
  );
}
