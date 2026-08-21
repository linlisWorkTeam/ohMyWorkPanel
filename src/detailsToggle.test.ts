import { describe, expect, it, vi } from "vitest";
import { applySelfDetailsToggle } from "./detailsToggle";

function detailsStub(open: boolean) {
  const node: { open: boolean; closest: (sel: string) => unknown } = {
    open,
    closest(sel: string) {
      return sel === "details" ? node : null;
    },
  };
  return node as unknown as EventTarget & { open: boolean };
}

describe("applySelfDetailsToggle", () => {
  it("ignores bubbled toggle from nested details (parent must not collapse)", () => {
    const setOpen = vi.fn();
    const inner = detailsStub(true);
    const outer = detailsStub(true);
    applySelfDetailsToggle(
      { target: inner, currentTarget: outer, stopPropagation() { /* no-op */ } },
      setOpen,
    );
    expect(setOpen).not.toHaveBeenCalled();
  });

  it("applies open from the details that actually toggled", () => {
    const setOpen = vi.fn();
    const self = detailsStub(true);
    applySelfDetailsToggle(
      { target: self, currentTarget: self, stopPropagation() { /* no-op */ } },
      setOpen,
    );
    expect(setOpen).toHaveBeenCalledWith(true);
  });

  it("treats a click whose target is the inner summary as self toggle", () => {
    const setOpen = vi.fn();
    const self = detailsStub(true);
    const summary = { closest: (sel: string) => (sel === "details" ? self : null) } as unknown as EventTarget;
    applySelfDetailsToggle(
      { target: summary, currentTarget: self, stopPropagation() { /* no-op */ } },
      setOpen,
    );
    expect(setOpen).toHaveBeenCalledWith(true);
  });
});
