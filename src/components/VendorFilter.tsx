import { useState, type CSSProperties, type MouseEvent } from "react";
import { Badge, Button, Card, Checkbox, Input, Radio, Space } from "antd";
import { MenuFoldOutlined, MenuUnfoldOutlined, SearchOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import type { VendorStat } from "../types";

const EXPANDED_WIDTH = 224;
const COLLAPSED_WIDTH = 56;

/** 行容器：整行可点，左侧控件 + 右侧计数。计数不放进控件 children——antd 5.29 的 Checkbox/Radio
 *  不支持语义化 styles，标签内容撑不满整行，做不出“名称左、计数右” */
const rowStyle: CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "6px 8px",
  borderRadius: 6,
  cursor: "pointer",
};

/** 行级点击：点在控件标签内时交给控件自己的 change 处理，避免一次点击切换两次 */
function rowClick(action: () => void) {
  return (e: MouseEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest("label")) return;
    action();
  };
}

/** 有货/总数计数：有货绿、售罄灰 */
function StockCount({ ok, total }: { ok: number; total: number }) {
  return (
    <span style={{ fontSize: 12, color: ok > 0 ? "#52c41a" : "#999" }}>
      {ok}/{total}
    </span>
  );
}

/** 商家筛选栏：多选商家 + 一键全部（已选为空即全部），可搜索、可收起。
 *  勾选结果经 onChange 回传，已选商家由调用方持有 */
export function VendorFilter({
  vendors,
  selected,
  onChange,
}: {
  /** 后端已按套餐数降序；加载失败时为空数组 */
  vendors: VendorStat[];
  /** 已选商家 key；空数组 = 全部（ALL） */
  selected: string[];
  /** 勾选/取消/点「全部」后回传新的已选数组 */
  onChange: (next: string[]) => void;
}) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);
  const [keyword, setKeyword] = useState("");

  const kw = keyword.trim().toLowerCase();
  const shown = kw
    ? vendors.filter(
        (v) => v.name.toLowerCase().includes(kw) || v.key.toLowerCase().includes(kw),
      )
    : vendors;
  const all = vendors.reduce(
    (acc, v) => ({ ok: acc.ok + v.ok, total: acc.total + v.total }),
    { ok: 0, total: 0 },
  );

  const toggle = (key: string) =>
    onChange(selected.includes(key) ? selected.filter((k) => k !== key) : [...selected, key]);
  // 已是全部时不再回调，否则页面会拿一个新的空数组重查一次
  const selectAll = () => {
    if (selected.length > 0) onChange([]);
  };

  const toggleLabel = collapsed ? t("products.expandSidebar") : t("products.collapseSidebar");
  const toggleButton = (
    <Button
      type="text"
      size="small"
      icon={collapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
      aria-label={toggleLabel}
      title={toggleLabel}
      onClick={() => setCollapsed((c) => !c)}
    />
  );
  // 已选数量在两种状态下都要可见：收起或搜索隐藏了已选商家时，结果仍被筛选
  const countBadge = <Badge count={selected.length} size="small" />;

  return (
    <div
      style={{
        width: collapsed ? COLLAPSED_WIDTH : EXPANDED_WIDTH,
        flexShrink: 0,
        // 页面滚动时侧栏固定可见（同设置页左菜单）
        position: "sticky",
        top: 24,
        alignSelf: "flex-start",
      }}
    >
      <Card
        size="small"
        // 高度不超过视口，商家多时在栏内滚动
        style={{ display: "flex", flexDirection: "column", maxHeight: "calc(100vh - 48px)" }}
        styles={{
          body: { display: "flex", flexDirection: "column", flex: 1, minHeight: 0 },
        }}
        title={
          collapsed ? undefined : (
            <Space size={6}>
              <span>{t("products.vendor")}</span>
              {countBadge}
            </Space>
          )
        }
        extra={
          collapsed ? (
            <Badge count={selected.length} size="small">
              {toggleButton}
            </Badge>
          ) : (
            toggleButton
          )
        }
      >
        {collapsed ? (
          <div style={{ display: "flex", justifyContent: "center" }}>
            <span
              style={{
                writingMode: "vertical-lr",
                letterSpacing: 4,
                fontSize: 12,
                color: "#999",
              }}
            >
              {t("products.vendorFilter")}
            </span>
          </div>
        ) : (
          <>
            <Input
              allowClear
              prefix={<SearchOutlined />}
              placeholder={t("products.searchVendors")}
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
            />
            <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
              <div
                style={{ ...rowStyle, borderBottom: "1px solid #f0f0f0" }}
                onClick={rowClick(selectAll)}
              >
                <Radio checked={selected.length === 0} onChange={selectAll}>
                  {t("products.allVendors")}
                </Radio>
                {/* 列表未加载到时不显示计数，避免闪一下 0/0 */}
                {vendors.length > 0 && <StockCount ok={all.ok} total={all.total} />}
              </div>
              {shown.map((v) => {
                const on = selected.includes(v.key);
                return (
                  <div
                    key={v.key}
                    style={{ ...rowStyle, background: on ? "#e6f4ff" : undefined }}
                    onClick={rowClick(() => toggle(v.key))}
                  >
                    <Checkbox checked={on} onChange={() => toggle(v.key)}>
                      <span style={on ? { color: "#1677ff", fontWeight: 600 } : undefined}>
                        {v.name}
                      </span>
                    </Checkbox>
                    <StockCount ok={v.ok} total={v.total} />
                  </div>
                );
              })}
              {kw && shown.length === 0 && (
                <div style={{ padding: 8, fontSize: 12, color: "#999" }}>{t("common.empty")}</div>
              )}
            </div>
          </>
        )}
      </Card>
    </div>
  );
}
