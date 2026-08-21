import { useEffect } from "react";

const DRAFT_PREFIX = "lp.composer.";

/**
 * composer 草稿按群持久化（localStorage；草稿/几何不进 DB）。
 * 切群自动恢复；发送后由调用方 setValue("") 清空（写入空稿）。
 */
export function useComposerDraft(groupId: string | undefined, value: string, setValue: (v: string) => void): void {
  useEffect(() => {
    if (!groupId) return;
    let loaded = "";
    try {
      loaded = localStorage.getItem(`${DRAFT_PREFIX}${groupId}`) ?? "";
    } catch {
      /* ignore */
    }
    setValue(loaded);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [groupId]);
  useEffect(() => {
    if (!groupId) return;
    const timer = window.setTimeout(() => {
      try {
        localStorage.setItem(`${DRAFT_PREFIX}${groupId}`, value);
      } catch {
        /* ignore */
      }
    }, 350);
    return () => window.clearTimeout(timer);
  }, [groupId, value]);
}
