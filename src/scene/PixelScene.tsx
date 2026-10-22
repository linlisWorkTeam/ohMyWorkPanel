import { useEffect, useRef, type MouseEvent } from "react";
import { agentKindOf, type SceneAgent } from "./sceneModel";

/** 虚拟画布尺寸；真实像素由容器缩放决定。 */
const VIEW_WIDTH = 720;
const VIEW_HEIGHT = 520;

interface Palette {
  bg: string;
  surf: string;
  elev: string;
  ink: string;
  dim: string;
  acc: string;
  ok: string;
  warning: string;
  critical: string;
  line: string;
}

/** 颜色一律从主题 token 读取，保持颜色门禁（零字面量）。 */
function readPalette(element: HTMLElement): Palette {
  const style = getComputedStyle(element);
  const read = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;
  return {
    bg: read("--bg", "transparent"),
    surf: read("--surf", "transparent"),
    elev: read("--elev", "transparent"),
    ink: read("--ink", "currentColor"),
    dim: read("--dim", "currentColor"),
    acc: read("--acc", "currentColor"),
    ok: read("--ok", "currentColor"),
    warning: read("--warning", "currentColor"),
    critical: read("--critical", "currentColor"),
    line: read("--line", "currentColor"),
  };
}

function stateColor(agent: SceneAgent, palette: Palette): string {
  switch (agent.state) {
    case "working":
      return palette.acc;
    case "blocked":
      return palette.critical;
    case "done":
      return palette.ok;
    default:
      return palette.dim;
  }
}

function drawFloor(ctx: CanvasRenderingContext2D, palette: Palette, roomSeed: number) {
  ctx.fillStyle = palette.bg;
  ctx.fillRect(0, 0, VIEW_WIDTH, VIEW_HEIGHT);

  // 房间色带：同一群 seed 固定，不同群分区位置不同（纯装饰，不影响交互）。
  ctx.save();
  ctx.globalAlpha = 0.05;
  ctx.fillStyle = palette.acc;
  for (let index = 0; index < 4; index += 1) {
    const band = (roomSeed + index * 2) % 8;
    ctx.fillRect(band * 90, 60, 90, VIEW_HEIGHT - 60);
  }
  ctx.restore();

  ctx.save();
  ctx.globalAlpha = 0.18;
  ctx.strokeStyle = palette.line;
  ctx.lineWidth = 1;
  for (let x = 0; x <= VIEW_WIDTH; x += 40) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, VIEW_HEIGHT);
    ctx.stroke();
  }
  for (let y = 0; y <= VIEW_HEIGHT; y += 40) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(VIEW_WIDTH, y);
    ctx.stroke();
  }
  ctx.restore();

  // 顶部墙面
  ctx.fillStyle = palette.surf;
  ctx.fillRect(0, 0, VIEW_WIDTH, 56);
  ctx.fillStyle = palette.elev;
  ctx.fillRect(0, 56, VIEW_WIDTH, 4);
}

function drawDesk(ctx: CanvasRenderingContext2D, agent: SceneAgent, palette: Palette) {
  ctx.fillStyle = palette.elev;
  ctx.fillRect(agent.x - 18, agent.y + 14, 44, 12);
  ctx.fillStyle = palette.line;
  ctx.fillRect(agent.x - 18, agent.y + 14, 44, 2);
}

function drawAgent(
  ctx: CanvasRenderingContext2D,
  agent: SceneAgent,
  palette: Palette,
  time: number,
  selected: boolean,
) {
  const color = stateColor(agent, palette);
  const bob =
    agent.state === "working"
      ? Math.sin(time / 220 + agent.seat) * 1.5
      : agent.state === "idle"
        ? Math.sin(time / 900 + agent.seat) * 0.6
        : 0;
  const y = agent.y + bob;

  ctx.save();
  if (agent.state === "blocked") {
    ctx.globalAlpha = 0.55 + 0.45 * Math.abs(Math.sin(time / 260));
  }

  // 影子
  ctx.fillStyle = palette.surf;
  ctx.fillRect(agent.x - 7, agent.y + 12, 14, 3);
  // 腿
  ctx.fillStyle = palette.elev;
  ctx.fillRect(agent.x - 4, y + 6, 3, 5);
  ctx.fillRect(agent.x + 1, y + 6, 3, 5);
  // 身体
  ctx.fillStyle = color;
  ctx.fillRect(agent.x - 5, y - 6, 10, 13);
  // 头
  ctx.fillStyle = palette.surf;
  ctx.fillRect(agent.x - 5, y - 16, 10, 9);
  // 眼睛
  ctx.fillStyle = palette.ink;
  ctx.fillRect(agent.x - 3, y - 13, 2, 2);
  ctx.fillRect(agent.x + 1, y - 13, 2, 2);

  // 头饰/配件：不同 adapter 的像素造型区分（纯装饰，不参与交互）。
  switch (agentKindOf(agent.adapter)) {
    case "codex":
      ctx.fillStyle = palette.acc;
      ctx.fillRect(agent.x - 6, y - 18, 12, 2);
      break;
    case "claude":
      ctx.fillStyle = palette.warning;
      ctx.fillRect(agent.x - 3, y - 19, 6, 3);
      break;
    case "opencode":
      ctx.fillStyle = palette.ok;
      ctx.fillRect(agent.x - 1, y - 21, 2, 5);
      break;
    case "cursor":
      ctx.fillStyle = palette.critical;
      ctx.fillRect(agent.x - 7, y - 14, 2, 4);
      break;
    case "dsh":
      ctx.fillStyle = palette.dim;
      ctx.fillRect(agent.x - 8, y - 15, 2, 3);
      ctx.fillRect(agent.x + 6, y - 15, 2, 3);
      break;
    case "mock":
      ctx.fillStyle = palette.elev;
      ctx.fillRect(agent.x - 3, y - 12, 6, 2);
      break;
    default:
      break;
  }

  if (agent.state === "done") {
    ctx.fillStyle = palette.ok;
    ctx.fillRect(agent.x + 6, y - 20, 5, 5);
  }
  if (agent.state === "working") {
    ctx.fillStyle = palette.acc;
    ctx.fillRect(agent.x + 7, y - 9, 3, 3);
  }
  if (selected) {
    ctx.strokeStyle = palette.acc;
    ctx.lineWidth = 1.5;
    ctx.strokeRect(agent.x - 9, y - 21, 18, 36);
  }
  ctx.restore();
}

function drawLabel(ctx: CanvasRenderingContext2D, agent: SceneAgent, palette: Palette) {
  ctx.font = "10px 'IBM Plex Mono', Consolas, monospace";
  ctx.textAlign = "center";
  const textWidth = ctx.measureText(agent.name).width;
  // 品牌色小方块：adapter 一眼可辨（比头饰更耐缩放）。
  ctx.fillStyle = kindColor(agent.adapter, palette);
  ctx.fillRect(agent.x - textWidth / 2 - 7, agent.y - 28, 4, 4);
  ctx.fillStyle = palette.dim;
  ctx.fillText(agent.name, agent.x, agent.y - 24);
}

/** adapter → 品牌色（全部走主题 token，保持颜色门禁）。 */
function kindColor(adapter: string, palette: Palette): string {
  switch (agentKindOf(adapter)) {
    case "codex":
      return palette.acc;
    case "claude":
      return palette.warning;
    case "opencode":
      return palette.ok;
    case "cursor":
      return palette.critical;
    case "dsh":
      return palette.dim;
    case "mock":
      return palette.elev;
    default:
      return palette.line;
  }
}

interface PixelSceneProps {
  agents: SceneAgent[];
  selectedId?: string | null;
  onSelectAgent?: (agentId: string) => void;
  /** 房间种子（由群 id 派生）：决定地板色带分区，0..7。 */
  roomSeed?: number;
}

export function PixelScene({ agents, selectedId, onSelectAgent, roomSeed = 0 }: PixelSceneProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const agentsRef = useRef<SceneAgent[]>(agents);
  const selectedRef = useRef<string | null>(selectedId ?? null);
  const viewRef = useRef({ scale: 1, offsetX: 0, offsetY: 0 });

  useEffect(() => {
    agentsRef.current = agents;
  }, [agents]);

  useEffect(() => {
    selectedRef.current = selectedId ?? null;
  }, [selectedId]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let frame = 0;
    let palette = readPalette(wrap);

    // 静态地板只画一次，后续每帧直接 blit，减少重复绘制。
    const floor = document.createElement("canvas");
    floor.width = VIEW_WIDTH;
    floor.height = VIEW_HEIGHT;
    const floorCtx = floor.getContext("2d");
    const renderFloor = () => {
      if (floorCtx) drawFloor(floorCtx, palette, roomSeed);
    };

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const width = Math.max(1, wrap.clientWidth);
      const height = Math.max(1, wrap.clientHeight);
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      const scale = Math.min(width / VIEW_WIDTH, height / VIEW_HEIGHT);
      viewRef.current = {
        scale,
        offsetX: (width - VIEW_WIDTH * scale) / 2,
        offsetY: (height - VIEW_HEIGHT * scale) / 2,
      };
      palette = readPalette(wrap);
      renderFloor();
    };

    const frameInterval = 1000 / 30;
    let lastPaint = 0;
    const draw = (time: number) => {
      frame = requestAnimationFrame(draw);
      if (time - lastPaint < frameInterval) return;
      lastPaint = time;

      const dpr = window.devicePixelRatio || 1;
      const { scale, offsetX, offsetY } = viewRef.current;
      ctx.setTransform(dpr * scale, 0, 0, dpr * scale, dpr * offsetX, dpr * offsetY);
      if (floorCtx) {
        ctx.drawImage(floor, 0, 0);
      } else {
        drawFloor(ctx, palette, roomSeed);
      }
      const agentsNow = agentsRef.current;
      for (const agent of agentsNow) {
        if (agent.state !== "idle") drawDesk(ctx, agent, palette);
      }
      const ordered = [...agentsNow].sort((a, b) => a.y - b.y);
      for (const agent of ordered) {
        drawAgent(ctx, agent, palette, time, selectedRef.current === agent.id);
        drawLabel(ctx, agent, palette);
      }
    };

    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(wrap);
    const themeObserver = new MutationObserver(() => {
      palette = readPalette(wrap);
      renderFloor();
    });
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    frame = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      themeObserver.disconnect();
    };
  }, [roomSeed]);

  const handleClick = (event: MouseEvent<HTMLCanvasElement>) => {
    if (!onSelectAgent) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const { scale, offsetX, offsetY } = viewRef.current;
    const x = (event.clientX - rect.left - offsetX) / scale;
    const y = (event.clientY - rect.top - offsetY) / scale;
    const hit = agentsRef.current.find(
      (agent) => Math.abs(agent.x - x) <= 16 && Math.abs(agent.y - 8 - y) <= 22,
    );
    if (hit) onSelectAgent(hit.id);
  };

  return (
    <div ref={wrapRef} className="scene-canvas-wrap">
      <canvas ref={canvasRef} className="scene-canvas" onClick={handleClick} />
    </div>
  );
}
