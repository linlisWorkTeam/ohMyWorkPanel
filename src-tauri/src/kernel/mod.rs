//! Plugin kernel v0.
//!
//! 语义对齐 Cordis 的最小可用子集：
//! - **Context 是服务容器**：服务以稳定 key 注册，按类型取回，不直接依赖具体实现。
//! - **inject 声明依赖**：插件先声明需要哪些 key，缺失时能一次列出全部缺口，而不是启动到一半才崩。
//! - **注册是可逆副作用**：所有注册配套 disposer，卸载时按逆序回收。
//!
//! 事件分发（emit / waterfall / parallel / serial / bail）与插件清单 v2
//! 属于后续切片，本模块暂不实现。

mod context;
mod effect;

pub use context::{Context, ServiceRegistryError};
pub use effect::EffectHandle;
