import { App, Button, Card, Space, Tag, Tooltip } from "antd";
import { CopyOutlined, StarFilled, StarOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { useState } from "react";
import type { ProductView } from "../types";
import { watchToggle } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { formatRelative } from "../time";

/** 库存卡片：状态、价格、地区、优惠码复制、关注切换、购买链接 */
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

  const price = product.price || (product.priceUsd != null ? `$${product.priceUsd.toFixed(2)}` : "-");

  return (
    <Card
      size="small"
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
      styles={{ body: { paddingTop: 8, paddingBottom: 8 } }}
    >
      <div style={{ fontWeight: 600, marginBottom: 4 }}>{product.name}</div>
      <Space size={4} wrap style={{ marginBottom: 4 }}>
        {(product.regionCn || product.region) && (
          <Tag bordered={false}>{product.regionFlag ? `${product.regionFlag.toUpperCase()} ` : ""}{product.regionCn || product.region}</Tag>
        )}
        {product.seriesName && <Tag bordered={false}>{product.seriesName}</Tag>}
        {product.route && <Tag bordered={false}>{t("products.route")}: {product.route}</Tag>}
      </Space>
      <Space size={8} wrap style={{ marginBottom: 4 }}>
        <span style={{ fontSize: 16, fontWeight: 600, color: "#1677ff" }}>{price}</span>
        {product.promo && (
          <Tooltip title={`${t("products.promo")}: ${product.promo}`}>
            <Tag color="volcano" style={{ cursor: "pointer" }} onClick={copyPromo}>
              <CopyOutlined /> {product.promo}
            </Tag>
          </Tooltip>
        )}
      </Space>
      <div style={{ color: "#999", fontSize: 12 }}>
        {t("products.lastChange")}: {formatRelative(product.lastChange ?? product.firstSeen)}
        {product.buyUrl && (
          <Button
            type="link"
            size="small"
            style={{ padding: 0, marginLeft: 8 }}
            onClick={() => void window.open(product.buyUrl, "_blank", "noopener")}
          >
            {t("products.buy")}
          </Button>
        )}
      </div>
    </Card>
  );
}
