import { HashRouter, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { App as AntApp, ConfigProvider, Layout, Menu } from "antd";
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

function AppLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Sider theme="dark" width={200}>
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
