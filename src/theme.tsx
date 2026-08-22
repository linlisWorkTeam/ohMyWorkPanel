import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

export type ThemeId = "atlas" | "cyberpunk" | "industrial" | "forge" | "moss" | "noir" | "minimal";

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
    blurb: "????",
    vibe: "???? ? ???? ? ???",
    swatch: ["#0a0014", "#ff2bd6", "#00f0ff"],
    featured: true,
  },
  {
    id: "industrial",
    name: "Industrial",
    blurb: "????",
    vibe: "???? ? ??? ? ???",
    swatch: ["#1a1c20", "#f5a623", "#ff6a00"],
    featured: true,
  },
  {
    id: "atlas",
    name: "Atlas",
    blurb: "?????",
    vibe: "??????",
    swatch: ["#e7eef5", "#0b1f33", "#1f9bb8"],
  },
  {
    id: "forge",
    name: "Forge",
    blurb: "????",
    vibe: "????",
    swatch: ["#141114", "#e08a3c", "#2a211c"],
  },
  {
    id: "moss",
    name: "Moss",
    blurb: "?????",
    vibe: "????",
    swatch: ["#e8efe6", "#1f3a2e", "#6a8f4e"],
  },
  {
    id: "noir",
    name: "Noir",
    blurb: "????",
    vibe: "?????",
    swatch: ["#0c0d10", "#f2ebe0", "#d94b3d"],
  },
  {
    id: "minimal",
    name: "??",
    blurb: "????",
    vibe: "???? ? ??? ? ???",
    swatch: ["#f4f5f7", "#17181a", "#5c5f66"],
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
    // bg-app ???????DSH ???meta theme-color ????????
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      const bg = getComputedStyle(document.documentElement).getPropertyValue("--bg-app").trim();
      meta.setAttribute("content", bg || "#202a3a");
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

/** Seven equal stage cards for ?? ? ??. */
export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme();
  return (
    <div className="wp-stage" role="group" aria-label="??????">
      {THEMES.map((item) => (
        <button
          key={item.id}
          type="button"
          className={`wp-theme-card${item.id === theme ? " on" : ""}`}
          aria-pressed={item.id === theme}
          onClick={() => setTheme(item.id)}
        >
          <span
            className="wp-theme-sw"
            style={{
              background: `linear-gradient(135deg, ${item.swatch[0]}, ${item.swatch[1]})`,
            }}
          />
          <b>{item.name}</b>
        </button>
      ))}
    </div>
  );
}

/** Header ?? popover ? compact 2-up grid matching docs/ui-demo.html */
export function HeaderThemePop({
  open,
  onPick,
}: {
  open: boolean;
  onPick: () => void;
}) {
  const { theme, setTheme } = useTheme();
  if (!open) return null;
  return (
    <div className="header-pop theme-pop" role="listbox" aria-label="????">
      <div className="header-theme-grid">
        {THEMES.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`ht-card ${item.id === theme ? "active" : ""}`}
            aria-selected={item.id === theme}
            onClick={() => {
              setTheme(item.id);
              onPick();
            }}
          >
            <span
              className="sw"
              style={{ background: `linear-gradient(135deg, ${item.swatch[0]} 0%, ${item.swatch[2]} 50%, ${item.swatch[1]} 100%)` }}
            />
            <b>{item.name}</b>
            <small>{item.blurb}</small>
          </button>
        ))}
      </div>
    </div>
  );
}
