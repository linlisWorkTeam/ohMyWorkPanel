//! 可撤销副作用：注册与回收的一等公民。
//!
//! 插件在安装阶段登记资源（监听器、后台任务、页签、定时器……），卸载时按
//! **逆序**释放。没有 disposer 的注册不应进入内核——否则热重载与卸载都会漏水。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

type Disposer = Box<dyn FnOnce() + Send>;

/// 一个副作用句柄。可用于观察是否已回收；真正的释放由上下文统一驱动。
#[derive(Debug)]
pub struct EffectHandle {
    id: u64,
    released: AtomicBool,
}

impl EffectHandle {
    pub(crate) fn new(id: u64) -> Self {
        Self {
            id,
            released: AtomicBool::new(false),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn is_released(&self) -> bool {
        self.released.load(Ordering::SeqCst)
    }

    pub(crate) fn mark_released(&self) {
        self.released.store(true, Ordering::SeqCst);
    }
}

struct RegisteredEffect {
    handle: std::sync::Arc<EffectHandle>,
    dispose: Disposer,
}

/// 逆序执行器：`dispose_all` 保证后注册的先释放。
#[derive(Default)]
pub(crate) struct EffectStack {
    effects: Mutex<Vec<RegisteredEffect>>,
    next_id: Mutex<u64>,
}

impl EffectStack {
    pub(crate) fn push(&self, dispose: Box<dyn FnOnce() + Send>) -> std::sync::Arc<EffectHandle> {
        let mut next_id = self.next_id.lock().expect("effect id lock poisoned");
        let id = *next_id;
        *next_id += 1;
        let handle = std::sync::Arc::new(EffectHandle::new(id));
        self.effects
            .lock()
            .expect("effect stack lock poisoned")
            .push(RegisteredEffect {
                handle: std::sync::Arc::clone(&handle),
                dispose,
            });
        handle
    }

    /// 逆序回收全部副作用；重复调用安全，已回收的不会再次执行。
    pub(crate) fn dispose_all(&self) {
        let effects = {
            let mut guard = self.effects.lock().expect("effect stack lock poisoned");
            std::mem::take(&mut *guard)
        };
        for effect in effects.into_iter().rev() {
            effect.handle.mark_released();
            (effect.dispose)();
        }
    }
}
