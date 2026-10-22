//! 场景能力的宿主侧解析：壳通过内核容器取用 `svc.scene`，不直接 import 视图。

import type { ComponentType } from "react";

import { frontendKernel } from "../kernel/host";
import { SCENE_SERVICE, type SceneCapability, type SceneGroupScope } from "./plugin";
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

/**
 * 为某个群开独立场景作用域（`ctx.fork()`）；插件缺失时返回 `null`。
 * 调用方在群切换/卸载时 MUST 调用 `scope.dispose()`。
 */
export function openSceneGroupScope(groupId: string): SceneGroupScope | null {
  try {
    return frontendKernel().require<SceneCapability>(SCENE_SERVICE).openGroupScope(groupId);
  } catch {
    return null;
  }
}
