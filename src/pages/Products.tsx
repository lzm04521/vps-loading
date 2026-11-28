import { useCallback, useEffect, useState } from "react";
import { App, Card, Checkbox, Input, InputNumber, Pagination, Segmented, Select, Space } from "antd";
import { useTranslation } from "react-i18next";
import type { ProductPage, RegionStat, VendorStat } from "../types";
import { productsList, regionsList, vendorsList } from "../api/ipc";
import { ipcErrorMessage } from "../i18n/errors";
import { ProductCard } from "../components/ProductCard";
import { VendorFilter } from "../components/VendorFilter";

const DEFAULT_PAGE_SIZE = 24;
const PAGE_SIZE_OPTIONS = [24, 48, 96, 200, 500, 1000];

export default function Products() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [page, setPage] = useState<ProductPage>({ total: 0, items: [] });
  const [vendors, setVendors] = useState<VendorStat[]>([]);
  const [regions, setRegions] = useState<RegionStat[]>([]);
  const [loading, setLoading] = useState(false);

  const [q, setQ] = useState("");
  const [status, setStatus] = useState("all");
  const [vendor, setVendor] = useState<string[]>([]);
  const [region, setRegion] = useState<string | undefined>();
  const [maxPrice, setMaxPrice] = useState<number | null>(null);
  const [watchOnly, setWatchOnly] = useState(false);
  const [sort, setSort] = useState("default");
  const [current, setCurrent] = useState(1);
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const result = await productsList({
        status,
        vendors: vendor.length ? vendor : null,
        region: region ?? null,
        q: q.trim() || null,
        minPrice: null,
        maxPrice,
        watch: watchOnly,
        sort,
        limit: pageSize,
        offset: (current - 1) * pageSize,
      });
      setPage(result);
    } catch (e) {
      message.error(ipcErrorMessage(e, t));
    } finally {
      setLoading(false);
    }
  }, [status, vendor, region, q, maxPrice, watchOnly, sort, current, pageSize, message, t]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    // 两份筛选项各自加载：一份失败不影响另一份；失败要提示，不能悄悄当成没有筛选项
    const load = async <T,>(request: () => Promise<T>, apply: (value: T) => void) => {
      try {
        apply(await request());
      } catch (e) {
        message.error(ipcErrorMessage(e, t));
      }
    };
    void load(vendorsList, setVendors);
    void load(regionsList, setRegions);
  }, [message, t]);

  const resetPage = () => setCurrent(1);

  const content = (
    <Space direction="vertical" size={12} style={{ flex: 1, minWidth: 0 }}>
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
        <div
          style={{
            display: "grid",
            // 按内容区宽度自动排列：每列至少 264px（卡片约 250px），最多 4 列；
            // 侧栏收放改变内容区宽度，列数自动增减
            gridTemplateColumns: "repeat(auto-fill, minmax(max(264px, calc(25% - 9px)), 1fr))",
            gap: 12,
          }}
        >
          {page.items.map((p) => (
            <ProductCard key={p.id} product={p} onWatchChanged={() => void reload()} />
          ))}
        </div>
      )}

      <Pagination
        current={current}
        pageSize={pageSize}
        total={page.total}
        showSizeChanger
        pageSizeOptions={PAGE_SIZE_OPTIONS}
        onChange={(p, sz) => {
          // 切换每页条数后回到第 1 页，避免高页码 offset 越界
          if (sz !== pageSize) {
            setPageSize(sz);
            setCurrent(1);
          } else {
            setCurrent(p);
          }
        }}
      />
    </Space>
  );

  return (
    <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
      <VendorFilter
        vendors={vendors}
        selected={vendor}
        onChange={(next) => {
          setVendor(next);
          resetPage();
        }}
      />
      {content}
    </div>
  );
}
