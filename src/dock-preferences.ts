export type DockSize = "compact" | "regular" | "large";
export type DockShape = "pill" | "rounded" | "square";
export type DockColor = "charcoal" | "graphite" | "plum" | "moss" | "rose";
export type DockPosition =
  | "bottom-center"
  | "bottom-left"
  | "bottom-right"
  | "top-center"
  | "top-left"
  | "top-right";

export interface DockPreferences {
  dockSize: DockSize;
  dockShape: DockShape;
  dockColor: DockColor;
  dockPosition: DockPosition;
  dockInset: number;
}

export const DOCK_SIZE_OPTIONS: Array<{ value: DockSize; label: string }> = [
  { value: "compact", label: "Compact" },
  { value: "regular", label: "Regular" },
  { value: "large", label: "Large" },
];

export const DOCK_SHAPE_OPTIONS: Array<{ value: DockShape; label: string }> = [
  { value: "pill", label: "Pill" },
  { value: "rounded", label: "Rounded" },
  { value: "square", label: "Square" },
];

export const DOCK_COLOR_OPTIONS: Array<{ value: DockColor; label: string }> = [
  { value: "charcoal", label: "Charcoal" },
  { value: "graphite", label: "Graphite" },
  { value: "plum", label: "Plum" },
  { value: "moss", label: "Moss" },
  { value: "rose", label: "Rose" },
];

export const DOCK_POSITION_OPTIONS: Array<{ value: DockPosition; label: string }> = [
  { value: "bottom-center", label: "Bottom Center" },
  { value: "bottom-left", label: "Bottom Left" },
  { value: "bottom-right", label: "Bottom Right" },
  { value: "top-center", label: "Top Center" },
  { value: "top-left", label: "Top Left" },
  { value: "top-right", label: "Top Right" },
];

export const DOCK_INSET_OPTIONS: Array<{ value: number; label: string }> = [
  { value: -80, label: "80 px outward" },
  { value: -70, label: "70 px outward" },
  { value: -60, label: "60 px outward" },
  { value: -50, label: "50 px outward" },
  { value: -40, label: "40 px outward" },
  { value: -30, label: "30 px outward" },
  { value: -20, label: "20 px outward" },
  { value: -10, label: "10 px outward" },
  { value: 0, label: "0 px" },
  { value: 10, label: "10 px inward" },
  { value: 20, label: "20 px inward" },
  { value: 30, label: "30 px inward" },
  { value: 40, label: "40 px inward" },
  { value: 50, label: "50 px inward" },
  { value: 60, label: "60 px inward" },
  { value: 70, label: "70 px inward" },
  { value: 80, label: "80 px inward" },
];

export const DEFAULT_DOCK_PREFERENCES: DockPreferences = {
  dockSize: "regular",
  dockShape: "pill",
  dockColor: "charcoal",
  dockPosition: "bottom-center",
  dockInset: 0,
};

function isOption<T extends string>(
  value: string | undefined,
  options: Array<{ value: T; label: string }>,
): value is T {
  return !!value && options.some((option) => option.value === value);
}

export function normalizeDockPreferences(
  preferences: Partial<DockPreferences>,
): DockPreferences {
  const dockInset = Number(preferences.dockInset);

  return {
    dockSize: isOption(preferences.dockSize, DOCK_SIZE_OPTIONS)
      ? preferences.dockSize
      : DEFAULT_DOCK_PREFERENCES.dockSize,
    dockShape: isOption(preferences.dockShape, DOCK_SHAPE_OPTIONS)
      ? preferences.dockShape
      : DEFAULT_DOCK_PREFERENCES.dockShape,
    dockColor: isOption(preferences.dockColor, DOCK_COLOR_OPTIONS)
      ? preferences.dockColor
      : DEFAULT_DOCK_PREFERENCES.dockColor,
    dockPosition: isOption(preferences.dockPosition, DOCK_POSITION_OPTIONS)
      ? preferences.dockPosition
      : DEFAULT_DOCK_PREFERENCES.dockPosition,
    dockInset: Number.isFinite(dockInset)
      ? Math.min(80, Math.max(-80, Math.round(dockInset / 10) * 10))
      : DEFAULT_DOCK_PREFERENCES.dockInset,
  };
}
