import { App, Button, Card, Space, Tag, Tooltip } from "antd";
import { CopyOutlined, StarFilled, StarOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useMemo, useState } from "react";
import type { ProductView } from "../types";
import { watchToggle } from "../api/ipc";
import { openExternal } from "../api/external";
import { ipcErrorMessage } from "../i18n/errors";
import { formatRelative } from "../time";
import { pickSpecEntries, SPEC_GRID_SIZE, type SpecEntry } from "../spec";
import { priceWithUnit } from "../price";

/** 规格六宫格单格：emoji 图标 + 字段名（灰小字）+ 值（加粗），对应图1 样式 */
function SpecCell({ icon, label, value }: SpecEntry) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6, minWidth: 0 }}>
      {icon && <span style={{ fontSize: 16, lineHeight: 1 }}>{icon}</span>}
      <div style={{ minWidth: 0 }}>
        <div style={{ color: "#999", fontSize: 11, lineHeight: "14px" }}>{label}</div>
        <div
          title={value}
          style={{
            fontSize: 13,
            fontWeight: 600,
            lineHeight: "16px",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {value}
        </div>
      </div>
    </div>
  );
}

/** 库存卡片：状态、规格六宫格、价格、地区、优惠码复制、关注切换、购买链接 */
export function ProductCard({
  product,
  onWatchChanged,
}: {
  product: ProductView;
  onWatchChanged?: () => void;
}) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [watched, setWatched] = useState(product.watched);
  const [toggling, setToggling] = useState(false);

  const spec = useMemo(() => pickSpecEntries(product.spec), [product.spec]);
  // 不足 6 格补空位，保持卡片高度一致
  const cells: (SpecEntry | null)[] = [
    ...spec.entries,
    ...Array.from({ length: SPEC_GRID_SIZE - spec.entries.length }, () => null),
  ];

  const toggleWatch = async () => {
    setToggling(true);
    try {
      const next = await watchToggle(product.id);
      setWatched(next);
      onWatchChanged?.();
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setToggling(false);
    }
  };

  const copyPromo = async () => {
    if (!product.promo) return;
    try {
      await navigator.clipboard.writeText(product.promo);
      message.success(t("common.copied"));
    } catch {
      message.error(product.promo);
    }
  };

  const openBuy = () => {
    void openExternal(product.buyUrl).catch((e) => message.error(String(e)));
  };

  // 空价回退美元折算价，折算参考价不带周期
  const price = product.price
    ? priceWithUnit(product.price, product.unit)
    : product.priceUsd != null
      ? `$${product.priceUsd.toFixed(2)}`
      : "-";

  const specGrid = (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(3, 1fr)",
        gap: "6px 8px",
        margin: "6px 0 8px",
      }}
    >
      {cells.map((c, i) =>
        c ? <SpecCell key={i} icon={c.icon} label={c.label} value={c.value} /> : <div key={i} />,
      )}
    </div>
  );

  return (
    <Card
      size="small"
      style={{ height: "100%", display: "flex", flexDirection: "column" }}
      styles={{ body: { paddingTop: 8, paddingBottom: 8, flex: 1, display: "flex", flexDirection: "column" } }}
      title={
        <Space size={4} wrap>
          <span style={{ fontWeight: 600 }}>{product.vendorName}</span>
          {product.status ? (
            <Tag color="green">{t("products.inStock")}</Tag>
          ) : (
            <Tag>{t("products.outOfStock")}</Tag>
          )}
        </Space>
      }
      extra={
        <Button
          type="text"
          size="small"
          loading={toggling}
          icon={watched ? <StarFilled style={{ color: "#faad14" }} /> : <StarOutlined />}
          onClick={toggleWatch}
          aria-label={t("products.watchOnly")}
        />
      }
    >
      <div
        title={product.name}
        style={{
          fontWeight: 600,
          marginBottom: 4,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {product.name}
      </div>
      {spec.extras.length > 0 ? (
        <Tooltip
          title={spec.extras.map((e) => `${e.label}: ${e.value}`).join("\n")}
          overlayInnerStyle={{ whiteSpace: "pre-line" }}
        >
          {specGrid}
        </Tooltip>
      ) : (
        specGrid
      )}
      <Space size={4} wrap style={{ minHeight: 22, marginBottom: 4 }}>
        {(product.regionCn || product.region) && (
          <Tag bordered={false}>{product.regionFlag ? `${product.regionFlag.toUpperCase()} ` : ""}{product.regionCn || product.region}</Tag>
        )}
        {product.seriesName && <Tag bordered={false}>{product.seriesName}</Tag>}
        {product.route && <Tag bordered={false}>{t("products.route")}: {product.route}</Tag>}
      </Space>
      <Space size={8} wrap align="center" style={{ minHeight: 24, marginBottom: 4 }}>
        <span style={{ fontSize: 16, fontWeight: 600, color: "#1677ff" }}>{price}</span>
        {product.promo && (
          <Tooltip title={`${t("products.promo")}: ${product.promo}`}>
            <Tag color="volcano" style={{ cursor: "pointer" }} onClick={copyPromo}>
              <CopyOutlined /> {product.promo}
            </Tag>
          </Tooltip>
        )}
      </Space>
      <div
        style={{
          marginTop: "auto",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          color: "#999",
          fontSize: 12,
        }}
      >
        <span>
          {t("products.lastChange")}: {formatRelative(product.lastChange ?? product.firstSeen)}
        </span>
        {product.buyUrl && (
          <Button type="link" size="small" style={{ padding: 0 }} onClick={openBuy}>
            {t("products.buy")}
          </Button>
        )}
      </div>
    </Card>
  );
}
