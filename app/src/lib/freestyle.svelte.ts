/** Per-key painting shared by the Freestyle panel (brush, base) and the stage (clicks). */
export const freestyle = $state({
  active: false,
  brush: "#2dd4bf",
  base: "#000000",
  keys: {} as Record<string, string>,
  /** Set by the panel: called after a key is painted so the change is saved. */
  onchange: (() => {}) as () => void,
});
