//! 场景能力的宿主侧解析：壳通过内核容器取用 `svc.scene`，不直接 import 视图。

import type { ComponentType } from "react";

import { frontendKernel } from "../kernel/host";
import { SCENE_SERVICE, type SceneCapability } from "./plugin";
import type { SceneViewProps } from "./SceneView";

/**
 * 取场景视图组件；插件未装载或装载失败时返回 `null`（壳回退到聊天视图）。
 */
export function resolveSceneView(): ComponentType<SceneViewProps> | null {
  try {
    return frontendKernel().require<SceneCapability>(SCENE_SERVICE).View;
  } catch {
    return null;
  }
}
