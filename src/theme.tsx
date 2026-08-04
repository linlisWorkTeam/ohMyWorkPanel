import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

export type ThemeId = "atlas" | "cyberpunk" | "industrial" | "forge" | "moss" | "noir";

export type ThemeMeta = {
  id: ThemeId;
  name: string;
  blurb: string;
  vibe: string;
  swatch: [string, string, string];
  featured?: boolean;
};

export const THEMES: ThemeMeta[] = [
  {
    id: "cyberpunk",
    name: "Cyberpunk",
    blurb: "霓虹夜城",
    vibe: "抖音赛博 · 粉青撞色 · 扫描线",
    swatch: ["#0a0014", "#ff2bd6", "#00f0ff"],
    featured: true,
  },
  {
    id: "industrial",
    name: "Industrial",
    blurb: "机械工坊",
    vibe: "钢板铆钉 · 警戒条 · 齿轮感",
    swatch: ["#1a1c20", "#f5a623", "#ff6a00"],
    featured: true,
  },
  {
    id: "atlas",
    name: "Atlas",
    blurb: "航图控制室",
    vibe: "清爽日间默认",
    swatch: ["#e7eef5", "#0b1f33", "#1f9bb8"],
  },
  {
    id: "forge",
    name: "Forge",
    blurb: "炭火工坊",
    vibe: "暗暖炭火",
    swatch: ["#141114", "#e08a3c", "#2a211c"],
  },
  {
    id: "moss",
    name: "Moss",
    blurb: "苔痕工作室",
    vibe: "柔和苔绿",
    swatch: ["#e8efe6", "#1f3a2e", "#6a8f4e"],
  },
  {
    id: "noir",
    name: "Noir",
    blurb: "墨稿夜台",
    vibe: "高对比暗色",
    swatch: ["#0c0d10", "#f2ebe0", "#d94b3d"],
  },
];

const STORAGE_KEY = "linlis_workpanel_theme";

type ThemeContextValue = {
  theme: ThemeId;
  setTheme: (id: ThemeId) => void;
};

const ThemeContext = createContext<ThemeContextValue | null>(null);

function readStoredTheme(): ThemeId {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    if (value && THEMES.some((theme) => theme.id === value)) {
      return value as ThemeId;
    }
  } catch {
    /* ignore */
  }
  return "cyberpunk";
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeId>(() => readStoredTheme());

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      /* ignore */
    }
  }, [theme]);

  const setTheme = (id: ThemeId) => setThemeState(id);

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}

export function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <div className={`brand ${compact ? "compact" : ""}`}>
      <span className="brand-mark" aria-hidden>
        <span className="brand-mark-core" />
      </span>
      <span className="brand-text">
        <span className="brand-linlis">Linli&apos;s</span>
        <span className="brand-workpanel">Workpanel</span>
      </span>
    </div>
  );
}

/** Fashion theme picker for 运行设置 — not the sidebar footer. */
export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme();
  const featured = THEMES.filter((t) => t.featured);
  const classic = THEMES.filter((t) => !t.featured);

  return (
    <div className="theme-switcher theme-switcher-settings" role="group" aria-label="切换界面主题">
      <div className="theme-switcher-label">外观主题</div>
      <p className="theme-switcher-hint">即时生效，保存在本机。主推赛博 / 机械工业，带背景图与动效。</p>
      <div className="theme-featured-grid">
        {featured.map((item) => {
          const active = item.id === theme;
          return (
            <button
              key={item.id}
              type="button"
              className={`theme-card theme-card-${item.id} ${active ? "active" : ""}`}
              aria-pressed={active}
              onClick={() => setTheme(item.id)}
            >
              <span
                className="theme-card-preview"
                style={{
                  backgroundImage: `linear-gradient(135deg, ${item.swatch[0]} 0%, ${item.swatch[2]} 45%, ${item.swatch[1]} 100%), url(/themes/${item.id === "cyberpunk" ? "cyberpunk" : "industrial"}.svg)`,
                  backgroundSize: "cover, cover",
                  backgroundBlendMode: "soft-light, normal",
                }}
              >
                <span className="theme-card-shine" aria-hidden />
                <span className="theme-card-badge">{item.blurb}</span>
              </span>
              <span className="theme-card-meta">
                <strong>{item.name}</strong>
                <small>{item.vibe}</small>
              </span>
            </button>
          );
        })}
      </div>
      <div className="theme-switcher-label theme-classic-label">经典</div>
      <div className="theme-swatches theme-swatches-classic">
        {classic.map((item) => {
          const active = item.id === theme;
          return (
            <button
              key={item.id}
              type="button"
              className={`theme-swatch ${active ? "active" : ""}`}
              aria-pressed={active}
              title={`${item.name} · ${item.blurb}`}
              onClick={() => setTheme(item.id)}
            >
              <span
                className="theme-swatch-face"
                style={{
                  background: `linear-gradient(135deg, ${item.swatch[0]} 0%, ${item.swatch[2]} 48%, ${item.swatch[1]} 100%)`,
                }}
              />
              <span className="theme-swatch-name">{item.name}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
