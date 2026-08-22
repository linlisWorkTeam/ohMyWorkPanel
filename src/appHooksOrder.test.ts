import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("App.tsx hook order vs auth early returns", () => {
  it("does not call hooks after session === login return (React #310)", () => {
    const src = readFileSync(resolve("src/App.tsx"), "utf8");
    const appStart = src.indexOf("export function App()");
    const authScreen = src.indexOf("function AuthScreen");
    expect(appStart).toBeGreaterThanOrEqual(0);
    expect(authScreen).toBeGreaterThan(appStart);
    const appFn = src.slice(appStart, authScreen);
    const loginReturn = appFn.search(/if\s*\(\s*requiresAuth\s*&&\s*session\s*===\s*"login"\s*\)/);
    expect(loginReturn).toBeGreaterThan(0);
    const after = appFn.slice(loginReturn);
    const hooks = after.match(/\buse(?:Effect|LayoutEffect|State|Callback|Memo|Ref|AppFrame)\s*\(/g) ?? [];
    expect(hooks).toEqual([]);
  });
});
