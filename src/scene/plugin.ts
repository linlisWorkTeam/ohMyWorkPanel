//! 像素场景的前端插件声明。
//!
//! 场景是「纯前台插件」：只读消费既有成员与 run 数据，不新增后端接口。
//! 通过 `svc.scene` 把视图组件交给壳，壳不再直接 import `SceneView`。

import type { ComponentType } from "react";

import type { PluginSpec } from "../kernel";
import { SceneView, type SceneViewProps } from "./SceneView";
import { roomSeedOf } from "./sceneModel";

export const SCENE_SERVICE = "svc.scene";
export const SCENE_GROUP_SERVICE = "svc.scene.group";
export const SCENE_PLUGIN_ID = "builtin.scene";

/** 单群作用域（`ctx.fork()` 派生）：承载群专属场景配置。 */
export interface SceneGroupScope {
  readonly groupId: string;
  /** 由群 id 派生的房间种子，决定地板分区。 */
  readonly roomSeed: number;
  dispose(): void;
}

export interface SceneCapability {
  readonly View: ComponentType<SceneViewProps>;
  /** 为某个群开一个独立场景作用域；用完 MUST dispose。 */
  openGroupScope(groupId: string): SceneGroupScope;
}

/** 场景插件：安装后提供 `svc.scene`。 */
export function scenePlugin(): PluginSpec {
  return {
    id: SCENE_PLUGIN_ID,
    provides: [SCENE_SERVICE],
    inject: [],
    install: (ctx) => {
      const capability: SceneCapability = {
        View: SceneView,
        openGroupScope: (groupId: string) => {
          const scope = ctx.fork();
          const roomSeed = roomSeedOf(groupId);
          scope.provide(SCENE_GROUP_SERVICE, { groupId, roomSeed });
          return {
            groupId,
            roomSeed,
            dispose: () => scope.dispose(),
          };
        },
      };
      ctx.provide(SCENE_SERVICE, capability);
    },
  };
}
