/** Nested <details> fire a bubbling `toggle`. Only the node that actually toggled should update React state. */
export function applySelfDetailsToggle(
  event: {
    target: EventTarget | null;
    currentTarget: EventTarget & { open: boolean };
    stopPropagation: () => void;
  },
  setOpen: (open: boolean) => void,
) {
  event.stopPropagation();
  const origin =
    event.target && typeof (event.target as Element).closest === "function"
      ? (event.target as Element).closest("details")
      : event.target;
  if (origin && origin !== event.currentTarget) return;
  setOpen(event.currentTarget.open);
}
