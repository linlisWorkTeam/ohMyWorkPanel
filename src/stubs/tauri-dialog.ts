// Stub for @tauri-apps/plugin-dialog — used in web builds only
export async function open(_options?: Record<string, unknown>): Promise<string | null> {
  // Fallback to native file input
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.onchange = () => {
      const file = input.files?.[0];
      resolve(file ? file.name : null);
    };
    input.click();
  });
}
export async function save(_options?: Record<string, unknown>): Promise<string | null> {
  throw new Error("Tauri save dialog is not available in browser mode");
}
export async function message(_message: string, _options?: Record<string, unknown>): Promise<void> {
  alert(_message);
}
export async function ask(_message: string, _options?: Record<string, unknown>): Promise<boolean> {
  return confirm(_message);
}
export async function confirm(_message: string, _options?: Record<string, unknown>): Promise<boolean> {
  return window.confirm(_message);
}
