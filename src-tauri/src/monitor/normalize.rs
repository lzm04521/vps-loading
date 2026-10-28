//! 源数据归一化：payload JSON → 领域草稿（vendors/series/products/history）。
//! 纯函数、无 IO；与旧版 source.py normalize 逐字段对齐。

use sha1::{Digest, Sha1};

#[derive(Debug, Clone, PartialEq)]
pub struct VendorDraft {
    pub key: String,
    pub name: String,
    pub logo: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SeriesDraft {
    pub vendor_key: String,
    pub key: String,
    pub name: String,
    pub region: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductDraft {
    pub vendor_key: String,
    pub series_key: String,
    pub series_name: String,
    pub name: String,
    pub price: String,
    pub price_usd: Option<f64>,
    pub region: String,
    pub region_cn: String,
    pub region_flag: String,
    pub unit: String,
    pub spec_json: String,
    pub route: String,
    pub ip_attr: String,
    pub promo: String,
    pub promo_exp: String,
    pub buy_url: String,
    pub buy_mode: String,
    pub blog_url: String,
    pub order_text: String,
    pub status: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryDraft {
    pub time_text: String,
    pub occurred_at: Option<String>,
    pub name: String,
    pub price: String,
    pub buy_url: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub vendors: Vec<VendorDraft>,
    pub series: Vec<SeriesDraft>,
    pub products: Vec<ProductDraft>,
    pub history: Vec<HistoryDraft>,
}

fn text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        v => match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        },
    }
}

/// route 字段专用转换：上游 data.js 部分商品已从字符串改为对象形态
/// （`{"ct":..,"cu":..,"cm":..}` 三网回程 / `{"single":..}` 单线路）。
/// 对象拼成与既有字符串风格一致的 `ct+cu+cm` 文本（重复值去重、空值跳过），
/// 避免 text() 对对象兜底 to_string() 把 JSON 原文当线路显示。
fn route_text(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s.trim().to_string(),
        Some(serde_json::Value::Object(map)) => {
            if let Some(single) = map.get("single") {
                return text(single).trim().to_string();
            }
            let mut parts: Vec<String> = Vec::new();
            for key in ["ct", "cu", "cm"] {
                let part = map.get(key).map(text).unwrap_or_default().trim().to_string();
                if !part.is_empty() && !parts.contains(&part) {
                    parts.push(part);
                }
            }
            parts.join("+")
        }
        Some(other) => other.to_string(),
    }
}

fn price_usd(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) if s.is_empty() => None,
        serde_json::Value::String(s) => s.parse().ok(),
        serde_json::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn status_int(value: &serde_json::Value) -> i64 {
    match value {
        serde_json::Value::Bool(b) => *b as i64,
        serde_json::Value::Number(n) if n.as_i64() == Some(1) => 1,
        serde_json::Value::String(s) => {
            let lower = s.trim().to_lowercase();
            if matches!(lower.as_str(), "1" | "true" | "yes" | "in" | "ok") {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// 源站时间为北京时间（%Y-%m-%d %H:%M），转成 UTC ISO-8601（秒精度，+00:00 后缀与旧库一致）
fn beijing_to_utc(time_text: &str) -> Option<String> {
    let naive =
        chrono::NaiveDateTime::parse_from_str(time_text.trim(), "%Y-%m-%d %H:%M").ok()?;
    let offset = chrono::FixedOffset::east_opt(8 * 3600)?;
    let utc = naive.and_local_timezone(offset).single()?.with_timezone(&chrono::Utc);
    Some(utc.to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
}

/// payload JSON → 领域快照。产品按 (vendor_key, series_key, name) 去重（后者覆盖前者）。
pub fn normalize(data: &serde_json::Value) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let mut products: std::collections::HashMap<(String, String, String), ProductDraft> =
        std::collections::HashMap::new();

    let empty = serde_json::Value::Null;
    let vendors = data.get("vendors").unwrap_or(&empty);
    for vendor in vendors.as_array().into_iter().flatten() {
        if !vendor.is_object() {
            continue;
        }
        let vendor_key = text(vendor.get("key").unwrap_or(&empty)).trim().to_string();
        if vendor_key.is_empty() || vendor_key == "all" {
            continue;
        }
        let vendor_name = {
            let n = text(vendor.get("name").unwrap_or(&empty)).trim().to_string();
            if n.is_empty() { vendor_key.clone() } else { n }
        };
        snapshot.vendors.push(VendorDraft {
            key: vendor_key.clone(),
            name: vendor_name,
            logo: text(vendor.get("logo").unwrap_or(&empty)),
        });

        for series in vendor
            .get("series")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            if !series.is_object() {
                continue;
            }
            let mut series_key = text(series.get("key").unwrap_or(&empty)).trim().to_string();
            let key_fallback = text(series.get("name").unwrap_or(&empty)).trim().to_string();
            if series_key.is_empty() {
                series_key = key_fallback;
            }
            let series_name = {
                let n = text(series.get("name").unwrap_or(&empty)).trim().to_string();
                if n.is_empty() { series_key.clone() } else { n }
            };
            if series_key.is_empty() {
                series_key = "默认".into();
            }
            let series_region = text(series.get("region").unwrap_or(&empty)).trim().to_string();
            snapshot.series.push(SeriesDraft {
                vendor_key: vendor_key.clone(),
                key: series_key.clone(),
                name: series_name.clone(),
                region: series_region.clone(),
            });

            for product in series
                .get("products")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                if !product.is_object() {
                    continue;
                }
                let name = text(product.get("name").unwrap_or(&empty)).trim().to_string();
                if name.is_empty() {
                    continue;
                }
                let region = {
                    let r = text(product.get("region").unwrap_or(&empty)).trim().to_string();
                    if r.is_empty() { series_region.clone() } else { r }
                };
                let region_cn = {
                    let r = text(product.get("region_cn").unwrap_or(&empty)).trim().to_string();
                    if r.is_empty() { region.clone() } else { r }
                };
                let spec = product.get("spec").filter(|v| v.is_object()).cloned().unwrap_or_else(|| serde_json::json!({}));
                let item = ProductDraft {
                    vendor_key: vendor_key.clone(),
                    series_key: series_key.clone(),
                    series_name: series_name.clone(),
                    name: name.clone(),
                    price: text(product.get("price").unwrap_or(&empty)),
                    price_usd: price_usd(product.get("price_usd").unwrap_or(&empty)),
                    region,
                    region_cn,
                    region_flag: text(product.get("region_flag").unwrap_or(&empty)),
                    unit: text(product.get("unit").unwrap_or(&empty)),
                    spec_json: serde_json::to_string(&spec).unwrap_or_else(|_| "{}".into()),
                    route: route_text(product.get("route")),
                    ip_attr: text(product.get("ip_attr").unwrap_or(&empty)),
                    promo: text(product.get("promo").unwrap_or(&empty)),
                    promo_exp: text(product.get("promo_exp").unwrap_or(&empty)),
                    buy_url: text(product.get("buy_url").unwrap_or(&empty)),
                    buy_mode: text(product.get("buy_mode").unwrap_or(&empty)),
                    blog_url: text(product.get("blog_url").unwrap_or(&empty)),
                    order_text: text(product.get("order").unwrap_or(&empty)),
                    status: status_int(product.get("status").unwrap_or(&empty)),
                };
                products.insert((vendor_key.clone(), series_key.clone(), name), item);
            }
        }
    }

    for item in data.get("history").and_then(|v| v.as_array()).into_iter().flatten() {
        if !item.is_object() {
            continue;
        }
        let name = text(item.get("name").unwrap_or(&empty)).trim().to_string();
        let time_text = text(item.get("time").unwrap_or(&empty)).trim().to_string();
        if name.is_empty() || time_text.is_empty() {
            continue;
        }
        let price = text(item.get("price").unwrap_or(&empty));
        let buy_url = text(item.get("buy_url").unwrap_or(&empty));
        let fingerprint = {
            let mut hasher = Sha1::new();
            hasher.update(format!("{time_text}|{name}|{price}|{buy_url}"));
            format!("{:x}", hasher.finalize())
        };
        snapshot.history.push(HistoryDraft {
            occurred_at: beijing_to_utc(&time_text),
            time_text,
            name,
            price,
            buy_url,
            fingerprint,
        });
    }

    snapshot.products = products.into_values().collect();
    snapshot
}

/// 跨模块测试共用的夹具
#[cfg(test)]
pub(crate) mod test_util {
    use super::*;

    pub fn sample_product_draft() -> ProductDraft {
        ProductDraft {
            vendor_key: "vultr".into(),
            series_key: "默认".into(),
            series_name: "默认".into(),
            name: "东京 1C1G".into(),
            price: "$5.00".into(),
            price_usd: Some(5.0),
            region: "JP".into(),
            region_cn: "日本东京".into(),
            region_flag: "jp".into(),
            unit: "月".into(),
            spec_json: r#"{"cpu":1,"mem":"1GB"}"#.into(),
            route: "软银".into(),
            ip_attr: String::new(),
            promo: "NEW10".into(),
            promo_exp: String::new(),
            buy_url: "https://example.com/buy".into(),
            buy_mode: String::new(),
            blog_url: String::new(),
            order_text: String::new(),
            status: 1,
        }
    }

    /// 模拟 kyoa data.js 的最小 payload（window.DATA 前缀剥除后的 JSON）
    pub fn sample_payload() -> serde_json::Value {
        serde_json::json!({
            "vendors": [
                {
                    "key": "all", "name": "全部", "series": []
                },
                {
                    "key": "vultr",
                    "name": "Vultr",
                    "logo": "https://example.com/logo.png",
                    "series": [
                        {
                            "key": "", "name": "", "region": "东京",
                            "products": [
                                {
                                    "name": "东京 1C1G",
                                    "price": "$5.00",
                                    "price_usd": 5.0,
                                    "region": "", "region_cn": "", "region_flag": "jp",
                                    "unit": "月",
                                    "spec": {"cpu": 1, "mem": "1GB"},
                                    "route": "软银", "promo": "NEW10",
                                    "buy_url": "https://example.com/buy",
                                    "status": true
                                },
                                {
                                    "name": "东京 2C2G",
                                    "price": "$10.00",
                                    "price_usd": 10.0,
                                    "region": "JP", "region_cn": "日本东京",
                                    "unit": "月", "spec": "not-a-map",
                                    "route": {"ct": "163", "cu": "4837", "cm": "CMI"}, "status": 0
                                }
                            ]
                        }
                    ]
                }
            ],
            "history": [
                {"time": "2026-01-02 10:00", "name": "东京 1C1G", "price": "$5.00", "buy_url": "https://example.com/buy"},
                {"name": "缺时间应被忽略"}
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_util::sample_payload;

    #[test]
    fn normalize_full_payload() {
        let snapshot = normalize(&sample_payload());
        // "all" 厂商被跳过
        assert_eq!(snapshot.vendors.len(), 1);
        assert_eq!(snapshot.vendors[0].key, "vultr");
        // 空 key/name 兜底为 "默认"，region 继承
        assert_eq!(snapshot.series.len(), 1);
        assert_eq!(snapshot.series[0].key, "默认");
        assert_eq!(snapshot.series[0].region, "东京");

        assert_eq!(snapshot.products.len(), 2);
        let p1 = snapshot
            .products
            .iter()
            .find(|p| p.name == "东京 1C1G")
            .unwrap();
        assert_eq!(p1.status, 1);
        assert_eq!(p1.price_usd, Some(5.0));
        assert_eq!(p1.region, "东京"); // 产品无 region 时继承系列
        assert_eq!(p1.region_cn, "东京"); // 无 region_cn 兜底 region
        assert!(p1.spec_json.contains("\"cpu\""));
        assert_eq!(p1.order_text, ""); // "order" 字段映射 order_text
        let p2 = snapshot
            .products
            .iter()
            .find(|p| p.name == "东京 2C2G")
            .unwrap();
        assert_eq!(p2.status, 0);
        assert_eq!(p2.spec_json, "{}"); // 非 object spec 兜底空对象
        assert_eq!(p2.route, "163+4837+CMI"); // 对象形态 route 拼成可读文本

        // 缺 time 的 history 被忽略；正常条目含 UTC 时间与 sha1 指纹
        assert_eq!(snapshot.history.len(), 1);
        let h = &snapshot.history[0];
        assert_eq!(h.occurred_at.as_deref(), Some("2026-01-02T02:00:00+00:00"));
        assert_eq!(h.fingerprint.len(), 40);
    }

    #[test]
    fn route_variants() {
        // 字符串形态原样透传（trim 首尾空白）
        assert_eq!(route_text(Some(&serde_json::json!(" 9929+CMIN2 "))), "9929+CMIN2");
        // 三网对象按 ct+cu+cm 顺序拼接（键序无关）
        assert_eq!(
            route_text(Some(&serde_json::json!({"cm": "CMI", "ct": "163", "cu": "4837"}))),
            "163+4837+CMI"
        );
        // 三值相同去重为单值
        assert_eq!(
            route_text(Some(&serde_json::json!({"ct": "CN2GIA", "cu": "CN2GIA", "cm": "CN2GIA"}))),
            "CN2GIA"
        );
        // 空值键跳过
        assert_eq!(
            route_text(Some(&serde_json::json!({"ct": "", "cu": "4837", "cm": ""}))),
            "4837"
        );
        // single 单线路取其值
        assert_eq!(route_text(Some(&serde_json::json!({"single": "普通直连"}))), "普通直连");
        // 空对象 / null / 缺失 → 空串
        assert_eq!(route_text(Some(&serde_json::json!({}))), "");
        assert_eq!(route_text(Some(&serde_json::Value::Null)), "");
        assert_eq!(route_text(None), "");
    }

    #[test]
    fn status_variants() {
        for (v, expected) in [
            (serde_json::json!(true), 1),
            (serde_json::json!(1), 1),
            (serde_json::json!("1"), 1),
            (serde_json::json!(" TRUE "), 1),
            (serde_json::json!("yes"), 1),
            (serde_json::json!(0), 0),
            (serde_json::json!(false), 0),
            (serde_json::json!("off"), 0),
            (serde_json::json!(null), 0),
        ] {
            assert_eq!(status_int(&v), expected, "value: {v}");
        }
    }

    #[test]
    fn price_usd_variants() {
        assert_eq!(price_usd(&serde_json::json!(5.5)), Some(5.5));
        assert_eq!(price_usd(&serde_json::json!("5.5")), Some(5.5));
        assert_eq!(price_usd(&serde_json::json!("")), None);
        assert_eq!(price_usd(&serde_json::json!("abc")), None);
        assert_eq!(price_usd(&serde_json::json!(null)), None);
    }
}
