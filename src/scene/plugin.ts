//! 像素场景的前端插件声明。
//!
//! 场景是「纯前台插件」：只读消费既有成员与 run 数据，不新增后端接口。
//! 通过 `svc.scene` 把视图组件交给壳，壳不再直接 import `SceneView`。

import type { ComponentType } from "react";

import type { PluginSpec } from "../kernel";
import { SceneView, type SceneViewProps } from "./SceneView";

export const SCENE_SERVICE = "svc.scene";
export const SCENE_PLUGIN_ID = "builtin.scene";

export interface SceneCapability {
  readonly View: ComponentType<SceneViewProps>;
}

/** 场景插件：安装后提供 `svc.scene`。 */
export function scenePlugin(): PluginSpec {
  return {
    id: SCENE_PLUGIN_ID,
    provides: [SCENE_SERVICE],
    inject: [],
    install: (ctx) => {
      const capability: SceneCapability = { View: SceneView };
      ctx.provide(SCENE_SERVICE, capability);
    },
  };
}
