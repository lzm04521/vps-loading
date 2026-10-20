import { useCallback, useEffect, useState } from "react";
import { App, Card, Checkbox, Col, Input, InputNumber, Pagination, Row, Segmented, Select, Space } from "antd";
import { useTranslation } from "react-i18next";
import type { ProductPage, RegionStat, VendorStat } from "../types";
import { productsList, regionsList, vendorsList } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { ProductCard } from "../components/ProductCard";

const PAGE_SIZE = 24;

export default function Products() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [page, setPage] = useState<ProductPage>({ total: 0, items: [] });
  const [vendors, setVendors] = useState<VendorStat[]>([]);
  const [regions, setRegions] = useState<RegionStat[]>([]);
  const [loading, setLoading] = useState(false);

  const [q, setQ] = useState("");
  const [status, setStatus] = useState("all");
  const [vendor, setVendor] = useState<string | undefined>();
  const [region, setRegion] = useState<string | undefined>();
  const [maxPrice, setMaxPrice] = useState<number | null>(null);
  const [watchOnly, setWatchOnly] = useState(false);
  const [sort, setSort] = useState("default");
  const [current, setCurrent] = useState(1);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const result = await productsList({
        status,
        vendor: vendor ?? null,
        region: region ?? null,
        q: q.trim() || null,
        minPrice: null,
        maxPrice,
        watch: watchOnly,
        sort,
        limit: PAGE_SIZE,
        offset: (current - 1) * PAGE_SIZE,
      });
      setPage(result);
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setLoading(false);
    }
  }, [status, vendor, region, q, maxPrice, watchOnly, sort, current, message, t]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    void (async () => {
      try {
        const [v, r] = await Promise.all([vendorsList(), regionsList()]);
        setVendors(v);
        setRegions(r);
      } catch {
        // 筛选项加载失败不阻塞主列表
      }
    })();
  }, []);

  const resetPage = () => setCurrent(1);

  return (
    <Space direction="vertical" size={12} style={{ width: "100%" }}>
      <Card size="small">
        <Space size={8} wrap>
          <Input.Search
            placeholder={t("products.searchPlaceholder")}
            allowClear
            style={{ width: 280 }}
            onSearch={(v) => {
              setQ(v);
              resetPage();
            }}
          />
          <Select
            style={{ width: 160 }}
            placeholder={t("products.vendor")}
            allowClear
            value={vendor}
            onChange={(v) => {
              setVendor(v);
              resetPage();
            }}
            options={[
              { value: "", label: t("products.allVendors") },
              ...vendors.map((v) => ({
                value: v.key,
                label: `${v.name} (${v.ok}/${v.total})`,
              })),
            ]}
          />
          <Select
            style={{ width: 150 }}
            placeholder={t("products.region")}
            allowClear
            value={region}
            onChange={(v) => {
              setRegion(v);
              resetPage();
            }}
            options={[
              { value: "", label: t("products.allRegions") },
              ...regions.map((r) => ({
                value: r.regionCn,
                label: `${r.regionCn} (${r.ok}/${r.total})`,
              })),
            ]}
          />
          <InputNumber
            placeholder={t("products.maxPrice")}
            min={0}
            step={0.01}
            value={maxPrice ?? undefined}
            onChange={(v) => {
              setMaxPrice(v ?? null);
              resetPage();
            }}
          />
          <Segmented
            value={status}
            onChange={(v) => {
              setStatus(v as string);
              resetPage();
            }}
            options={[
              { value: "all", label: t("products.statusAll") },
              { value: "in", label: t("products.statusIn") },
              { value: "out", label: t("products.statusOut") },
            ]}
          />
          <Checkbox
            checked={watchOnly}
            onChange={(e) => {
              setWatchOnly(e.target.checked);
              resetPage();
            }}
          >
            {t("products.watchOnly")}
          </Checkbox>
          <Select
            style={{ width: 170 }}
            value={sort}
            onChange={(v) => {
              setSort(v);
              resetPage();
            }}
            options={Object.entries({
              default: t("products.sorts.default"),
              price_asc: t("products.sorts.price_asc"),
              price_desc: t("products.sorts.price_desc"),
              change: t("products.sorts.change"),
              name: t("products.sorts.name"),
              vendor: t("products.sorts.vendor"),
            }).map(([value, label]) => ({ value, label }))}
          />
        </Space>
      </Card>

      <div>{t("products.resultCount", { total: page.total })}</div>

      {page.items.length === 0 && !loading ? (
        <Card size="small">{t("products.noResults")}</Card>
      ) : (
        <Row gutter={[12, 12]}>
          {page.items.map((p) => (
            <Col key={p.id} xs={24} sm={12} lg={8} xl={6}>
              <ProductCard product={p} onWatchChanged={() => void reload()} />
            </Col>
          ))}
        </Row>
      )}

      <Pagination
        current={current}
        pageSize={PAGE_SIZE}
        total={page.total}
        showSizeChanger={false}
        onChange={setCurrent}
      />
    </Space>
  );
}
