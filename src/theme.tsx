import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

export type ThemeId = "atlas" | "forge" | "moss" | "noir";

export type ThemeMeta = {
  id: ThemeId;
  name: string;
  blurb: string;
  swatch: [string, string, string];
};

export const THEMES: ThemeMeta[] = [
  {
    id: "atlas",
    name: "Atlas",
    blurb: "航图控制室",
    swatch: ["#e7eef5", "#0b1f33", "#1f9bb8"],
  },
  {
    id: "forge",
    name: "Forge",
    blurb: "炭火工坊",
    swatch: ["#141114", "#e08a3c", "#2a211c"],
  },
  {
    id: "moss",
    name: "Moss",
    blurb: "苔痕工作室",
    swatch: ["#e8efe6", "#1f3a2e", "#6a8f4e"],
  },
  {
    id: "noir",
    name: "Noir",
    blurb: "墨稿夜台",
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
  return "atlas";
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

export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme();
  return (
    <div className="theme-switcher" role="group" aria-label="切换界面主题">
      <div className="theme-switcher-label">主题</div>
      <div className="theme-swatches">
        {THEMES.map((item) => {
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
