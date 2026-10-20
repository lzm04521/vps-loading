//! 快照对比决策表（纯函数，无 IO）。
//! 旧版把 diff 内联在 collector._collect 里，这里抽成可单测的独立模块。

use crate::model::EventType;

/// 单个事件草稿（落库前）
#[derive(Debug, Clone, PartialEq)]
pub struct EventDraft {
    pub product_id: i64,
    pub event_type: EventType,
    pub old_status: Option<i64>,
    pub new_status: Option<i64>,
    pub old_price_usd: Option<f64>,
    pub new_price_usd: Option<f64>,
}

/// 价格变化方向；差值 < 0.005 视为未变（与旧版一致）
pub fn price_change_type(old_price: Option<f64>, new_price: Option<f64>) -> Option<EventType> {
    let (old, new) = (old_price?, new_price?);
    if (old - new).abs() < 0.005 {
        return None;
    }
    if new < old {
        Some(EventType::PriceDown)
    } else {
        Some(EventType::PriceUp)
    }
}

/// 对一个已存在的产品生成状态/价格事件（0~2 条）
pub fn product_events(
    product_id: i64,
    old_status: i64,
    new_status: i64,
    old_price_usd: Option<f64>,
    new_price_usd: Option<f64>,
) -> Vec<EventDraft> {
    let mut events = Vec::new();
    if old_status != new_status {
        let event_type = if new_status == 1 {
            EventType::Restock
        } else {
            EventType::Soldout
        };
        events.push(EventDraft {
            product_id,
            event_type,
            old_status: Some(old_status),
            new_status: Some(new_status),
            old_price_usd,
            new_price_usd,
        });
    }
    if let Some(event_type) = price_change_type(old_price_usd, new_price_usd) {
        events.push(EventDraft {
            product_id,
            event_type,
            old_status: Some(old_status),
            new_status: Some(new_status),
            old_price_usd,
            new_price_usd,
        });
    }
    events
}

/// 新产品事件
pub fn new_product_event(product_id: i64, new_status: i64, price_usd: Option<f64>) -> EventDraft {
    EventDraft {
        product_id,
        event_type: EventType::NewProduct,
        old_status: None,
        new_status: Some(new_status),
        old_price_usd: None,
        new_price_usd: price_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_change_directions() {
        assert_eq!(price_change_type(Some(5.0), Some(4.0)), Some(EventType::PriceDown));
        assert_eq!(price_change_type(Some(4.0), Some(5.0)), Some(EventType::PriceUp));
        // 微小变动视为未变
        assert_eq!(price_change_type(Some(5.0), Some(5.004)), None);
        assert_eq!(price_change_type(None, Some(4.0)), None);
        assert_eq!(price_change_type(Some(4.0), None), None);
        assert_eq!(price_change_type(None, None), None);
    }

    #[test]
    fn restock_and_soldout() {
        let e = product_events(1, 0, 1, None, None);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].event_type, EventType::Restock);
        assert_eq!(e[0].old_status, Some(0));

        let e = product_events(1, 1, 0, None, None);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].event_type, EventType::Soldout);

        assert!(product_events(1, 1, 1, Some(5.0), Some(5.0)).is_empty());
    }

    #[test]
    fn status_and_price_both_change() {
        let e = product_events(7, 0, 1, Some(10.0), Some(5.0));
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].event_type, EventType::Restock);
        assert_eq!(e[1].event_type, EventType::PriceDown);
        assert_eq!(e[1].product_id, 7);
    }

    #[test]
    fn new_product() {
        let e = new_product_event(3, 1, Some(4.2));
        assert_eq!(e.event_type, EventType::NewProduct);
        assert_eq!(e.old_status, None);
        assert_eq!(e.new_price_usd, Some(4.2));
    }
}
