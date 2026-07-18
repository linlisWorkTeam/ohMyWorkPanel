// Stub for @tauri-apps/api/core — used in web builds only
export function invoke(_cmd: string, _args?: Record<string, unknown>): Promise<unknown> {
  throw new Error("Tauri invoke is not available in browser mode");
}
export async function convertFileSrc(_path: string): Promise<string> {
  throw new Error("Tauri convertFileSrc is not available in browser mode");
}
export async function getCurrentWebviewWindow() {
  throw new Error("Tauri getCurrentWebviewWindow is not available in browser mode");
}
export const appWindow = undefined;
