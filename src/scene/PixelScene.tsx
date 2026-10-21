import { useEffect, useRef, type MouseEvent } from "react";
import type { SceneAgent } from "./sceneModel";

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

function drawFloor(ctx: CanvasRenderingContext2D, palette: Palette) {
  ctx.fillStyle = palette.bg;
  ctx.fillRect(0, 0, VIEW_WIDTH, VIEW_HEIGHT);

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

  if (agent.state === "done") {
    ctx.fillStyle = palette.ok;
    ctx.fillRect(agent.x + 6, y - 20, 5, 5);
  }
  if (agent.state === "working") {
    ctx.fillStyle = palette.acc;
    ctx.fillRect(agent.x + 7, y - 9, 3, 3);
  }
  ctx.restore();
}

function drawLabel(ctx: CanvasRenderingContext2D, agent: SceneAgent, palette: Palette) {
  ctx.fillStyle = palette.dim;
  ctx.font = "10px 'IBM Plex Mono', Consolas, monospace";
  ctx.textAlign = "center";
  ctx.fillText(agent.name, agent.x, agent.y - 24);
}

interface PixelSceneProps {
  agents: SceneAgent[];
  onSelectAgent?: (agentId: string) => void;
}

export function PixelScene({ agents, onSelectAgent }: PixelSceneProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const agentsRef = useRef<SceneAgent[]>(agents);
  const viewRef = useRef({ scale: 1, offsetX: 0, offsetY: 0 });

  useEffect(() => {
    agentsRef.current = agents;
  }, [agents]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let frame = 0;
    let palette = readPalette(wrap);

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
    };

    const draw = (time: number) => {
      const dpr = window.devicePixelRatio || 1;
      const { scale, offsetX, offsetY } = viewRef.current;
      ctx.setTransform(dpr * scale, 0, 0, dpr * scale, dpr * offsetX, dpr * offsetY);
      drawFloor(ctx, palette);
      const agentsNow = agentsRef.current;
      for (const agent of agentsNow) {
        if (agent.state !== "idle") drawDesk(ctx, agent, palette);
      }
      const ordered = [...agentsNow].sort((a, b) => a.y - b.y);
      for (const agent of ordered) {
        drawAgent(ctx, agent, palette, time);
        drawLabel(ctx, agent, palette);
      }
      frame = requestAnimationFrame(draw);
    };

    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(wrap);
    const themeObserver = new MutationObserver(() => {
      palette = readPalette(wrap);
    });
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    frame = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      themeObserver.disconnect();
    };
  }, []);

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
