//! 监控核心域：normalize（归一化）/ diff（决策表）/ engine（单次循环）/
//! service（常驻调度）/ alerts（规则与通道）。

pub mod alerts;
pub mod diff;
pub mod engine;
pub mod normalize;
pub mod service;
