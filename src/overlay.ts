import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  type DockPreferences,
  normalizeDockPreferences,
} from "./dock-preferences";
import {
  type TextTransform,
  getTransformIconSvg,
  normalizeTransforms,
  selectedTransform,
  statusLabelsForTransform,
} from "./transforms";

interface Settings {
  microphone: string;
  engine: string;
  whisperModel: string;
  groqApiKey: string;
  transcriptModelMode: "default" | "custom";
  transcriptModelId: string;
  transformModelMode: "default" | "custom";
  transformModelId: string;
  autoPolish: boolean;
  defaultTransform: string;
  transforms: TextTransform[];
  recordingMode: string;
  dockSize: DockPreferences["dockSize"];
  dockShape: DockPreferences["dockShape"];
  dockColor: DockPreferences["dockColor"];
  dockPosition: DockPreferences["dockPosition"];
  dockInset: DockPreferences["dockInset"];
  hotkey: string;
}

interface CursorPayload {
  x: number;
  y: number;
  inside: boolean;
}

interface AudioLevelPayload {
  level: number;
}

type VisualState = "ready" | "recording" | "transcribing" | "polishing" | "inserted" | "polished";
type OverlayLayout = "parked" | "dock" | "label" | "menu" | "recording" | "status";
type TipTarget = "dictate" | "transform" | "caret" | "notes";

declare global {
  interface Window {
    setTyprState?: (state: string) => void;
  }
}

const EXPAND_INTENT_MS = 90;
const COLLAPSE_GRACE_MS = 200;
const TIP_GRACE_MS = 140;
const WAVEFORM_BAR_COUNT = 24;

const body = document.body;
const collapsedHandle = document.getElementById("collapsed-handle") as HTMLButtonElement;
const dictateButton = document.getElementById("dictate-button") as HTMLButtonElement;
const transformButton = document.getElementById("transform-button") as HTMLButtonElement;
const transformButtonIcon = document.getElementById("transform-button-icon")!;
const transformCaret = document.getElementById("transform-caret") as HTMLButtonElement;
const transformOptionsList = document.getElementById("transform-options")!;
const notesButton = document.getElementById("notes-button") as HTMLButtonElement;
const configureTransforms = document.getElementById("configure-transforms") as HTMLButtonElement;
const autoApplyToggle = document.getElementById("auto-apply-toggle") as HTMLButtonElement;
const hoverLabel = document.getElementById("hover-label")!;
const polishingLabel = document.getElementById("polishing-label")!;
const polishedLabel = document.getElementById("polished-label")!;
const recordingTimer = document.getElementById("recording-timer")!;
const waveform = document.getElementById("waveform")!;
const cancelRecording = document.getElementById("cancel-recording") as HTMLButtonElement;
const stopRecording = document.getElementById("stop-recording") as HTMLButtonElement;

let settings: Settings | null = null;
let visualState: VisualState = "ready";
let expanded = false;
let menuOpen = false;
let tipTarget: TipTarget | null = null;
let hoveredElement: HTMLElement | null = null;
let expandTimer: number | undefined;
let collapseTimer: number | undefined;
let tipTimer: number | undefined;
let completionTimer: number | undefined;
let recordingStartTime = 0;
let recordingTimerInterval: number | undefined;
let waveformDecayInterval: number | undefined;
let waveformLevels = Array.from({ length: WAVEFORM_BAR_COUNT }, () => 0);
let currentLayoutKey: string | null = null;

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function formatHotkey(hotkey: string) {
  if (!hotkey.trim()) return "No hotkey";

  return hotkey
    .replace(/CmdOrCtrl/g, "⌘")
    .replace(/Command/g, "⌘")
    .replace(/Cmd/g, "⌘")
    .replace(/Ctrl/g, "⌃")
    .replace(/Control/g, "⌃")
    .replace(/Option/g, "⌥")
    .replace(/Alt/g, "⌥")
    .replace(/Key([A-Z])/g, "$1")
    .replace(/Digit([0-9])/g, "$1")
    .replace(/\+/g, "");
}

function transformLabels() {
  return statusLabelsForTransform(
    selectedTransform(settings?.transforms, settings?.defaultTransform),
  );
}

function normalizeState(state: string): VisualState {
  const normalized = String(state).toLowerCase();
  if (normalized === "recording") return "recording";
  if (normalized === "transcribing") return "transcribing";
  if (normalized === "polishing") return "polishing";
  if (normalized === "inserted") return "inserted";
  if (normalized === "polished") return "polished";
  return "ready";
}

function completionState(previous: VisualState): VisualState {
  if (previous === "polishing") return "polished";
  if (previous === "transcribing" || previous === "recording") return "inserted";
  return "ready";
}

/* ---------------------------------------------------------------------------
 * Layout sync — the Rust side resizes the transparent window per layout.
 * ------------------------------------------------------------------------- */

function overlayLayout(): OverlayLayout {
  if (visualState === "recording") return "recording";
  if (visualState !== "ready") return "status";
  if (!expanded) return "parked";
  if (menuOpen) return "menu";
  if (tipTarget) return "label";
  return "dock";
}

function syncOverlayLayout() {
  const layout = overlayLayout();
  const preferences = normalizeDockPreferences(settings ?? {});
  const layoutKey = `${layout}:${preferences.dockSize}:${preferences.dockPosition}:${preferences.dockInset}`;
  if (layoutKey === currentLayoutKey) return;
  currentLayoutKey = layoutKey;

  invoke("set_overlay_layout", { layout }).catch((error) => {
    console.error("Failed to resize overlay", error);
  });
}

function render() {
  const showDock = visualState === "ready" && expanded;
  body.classList.toggle("expanded", showDock);
  body.classList.toggle("show-popover", showDock && menuOpen);
  body.classList.toggle("show-label", showDock && !!tipTarget && !menuOpen);
  transformCaret.setAttribute("aria-expanded", String(showDock && menuOpen));
  syncOverlayLayout();
}

/* ---------------------------------------------------------------------------
 * Settings
 * ------------------------------------------------------------------------- */

function configuredTransforms() {
  return normalizeTransforms(settings?.transforms);
}

function applyDockPreferences() {
  const preferences = normalizeDockPreferences(settings ?? {});
  body.dataset.dockSize = preferences.dockSize;
  body.dataset.dockShape = preferences.dockShape;
  body.dataset.dockColor = preferences.dockColor;
  body.dataset.dockPosition = preferences.dockPosition;
}

function createTransformOption(transform: TextTransform, selectedId: string) {
  const option = document.createElement("button");
  option.className = "popover-row transform-option";
  option.type = "button";
  option.dataset.hov = "";
  option.dataset.transform = transform.id;
  option.setAttribute("aria-pressed", String(transform.id === selectedId));

  const label = document.createElement("span");
  label.className = "popover-transform-label";

  const icon = document.createElement("span");
  icon.className = "popover-transform-icon";
  icon.innerHTML = getTransformIconSvg(transform.icon);

  const name = document.createElement("span");
  name.textContent = transform.name;

  label.append(icon, name);

  const check = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  check.setAttribute("class", "check-icon");
  check.setAttribute("viewBox", "0 0 24 24");
  check.setAttribute("aria-hidden", "true");
  const checkPath = document.createElementNS("http://www.w3.org/2000/svg", "path");
  checkPath.setAttribute("d", "M5 12.5 9.5 17 19 7");
  check.append(checkPath);

  option.append(label, check);
  option.addEventListener("click", () => void pickTransform(transform.id));

  return option;
}

function renderTransformOptions() {
  const transforms = configuredTransforms();
  const selected = selectedTransform(transforms, settings?.defaultTransform);
  transformOptionsList.replaceChildren(
    ...transforms.map((transform) => createTransformOption(transform, selected.id)),
  );
}

function renderSettings() {
  applyDockPreferences();
  const autoApply = settings?.autoPolish ?? true;
  autoApplyToggle.setAttribute("aria-pressed", String(autoApply));

  const selected = selectedTransform(configuredTransforms(), settings?.defaultTransform);
  transformButtonIcon.innerHTML = getTransformIconSvg(selected.icon);
  renderTransformOptions();

  const labels = transformLabels();
  polishingLabel.textContent = labels.busy;
  polishedLabel.textContent = labels.done;

  if (tipTarget === "transform") {
    renderTip("transform");
  }

  syncOverlayLayout();
}

async function loadSettings() {
  settings = await invoke<Settings>("get_settings");
  renderSettings();
}

async function saveSettings(nextSettings: Settings) {
  const previous = settings;
  const normalizedSettings = {
    ...nextSettings,
    ...normalizeDockPreferences(nextSettings),
    transforms: normalizeTransforms(nextSettings.transforms),
  };
  settings = normalizedSettings;
  renderSettings();
  try {
    await invoke("save_settings", { settings: normalizedSettings });
  } catch (error) {
    console.error("Failed to save settings", error);
    settings = previous;
    renderSettings();
  }
}

/* ---------------------------------------------------------------------------
 * Tooltips
 * ------------------------------------------------------------------------- */

function renderTip(target: TipTarget) {
  let label = "";
  let accent = "";

  if (target === "dictate") {
    label = "Dictate";
    accent = formatHotkey(settings?.hotkey ?? "Ctrl+Option+Space");
  } else if (target === "transform") {
    label = transformLabels().name;
  } else if (target === "caret") {
    label = "Transforms";
  } else {
    label = "Notes";
  }

  hoverLabel.innerHTML = accent
    ? `${escapeHtml(label)} <strong>${escapeHtml(accent)}</strong>`
    : escapeHtml(label);
}

function showTip(target: TipTarget) {
  window.clearTimeout(tipTimer);
  tipTimer = undefined;
  if (visualState !== "ready" || menuOpen) return;
  if (tipTarget === target) return;
  tipTarget = target;
  renderTip(target);
  render();
}

function scheduleTipClear() {
  if (!tipTarget || tipTimer !== undefined) return;
  tipTimer = window.setTimeout(() => {
    tipTimer = undefined;
    tipTarget = null;
    render();
  }, TIP_GRACE_MS);
}

function clearTipNow() {
  window.clearTimeout(tipTimer);
  tipTimer = undefined;
  tipTarget = null;
}

/* ---------------------------------------------------------------------------
 * Expand / collapse
 * ------------------------------------------------------------------------- */

function setExpanded(next: boolean) {
  if (expanded === next) return;
  expanded = next;
  if (!next) {
    menuOpen = false;
    clearTipNow();
    setHoveredElement(null);
  }
  render();
}

function scheduleExpand() {
  if (expanded || expandTimer !== undefined) return;
  expandTimer = window.setTimeout(() => {
    expandTimer = undefined;
    setExpanded(true);
  }, EXPAND_INTENT_MS);
}

function cancelExpand() {
  window.clearTimeout(expandTimer);
  expandTimer = undefined;
}

function scheduleCollapse() {
  cancelExpand();
  if (!expanded || collapseTimer !== undefined) return;
  collapseTimer = window.setTimeout(() => {
    collapseTimer = undefined;
    setExpanded(false);
  }, COLLAPSE_GRACE_MS);
}

function cancelCollapse() {
  window.clearTimeout(collapseTimer);
  collapseTimer = undefined;
}

function setMenuOpen(next: boolean) {
  if (menuOpen === next) return;
  menuOpen = next;
  if (next) clearTipNow();
  render();
}

/* ---------------------------------------------------------------------------
 * Cursor pipeline — fed by the Rust global cursor watcher (works while the
 * window is unfocused, which macOS native hover does not) and by native
 * mouse events as a fallback when the window has focus.
 * ------------------------------------------------------------------------- */

function setHoveredElement(next: HTMLElement | null) {
  if (hoveredElement === next) return;
  hoveredElement?.classList.remove("hov");
  hoveredElement = next;
  hoveredElement?.classList.add("hov");
}

function hitTest(x: number, y: number): HTMLElement | null {
  const element = document.elementFromPoint(x, y);
  if (!element) return null;
  return element.closest<HTMLElement>("[data-hov]");
}

function handleCursor(x: number, y: number, inside: boolean) {
  if (!inside) {
    setHoveredElement(null);
    cancelExpand();
    scheduleTipClear();
    scheduleCollapse();
    return;
  }

  cancelCollapse();

  if (visualState !== "ready") {
    setHoveredElement(hitTest(x, y));
    return;
  }

  if (!expanded) {
    setHoveredElement(hitTest(x, y));
    scheduleExpand();
    return;
  }

  const target = hitTest(x, y);
  setHoveredElement(target);

  if (menuOpen) return;

  const tip = target?.closest<HTMLElement>("[data-tip]")?.dataset.tip as TipTarget | undefined;
  if (tip) {
    showTip(tip);
  } else {
    scheduleTipClear();
  }
}

/* ---------------------------------------------------------------------------
 * Recording UI
 * ------------------------------------------------------------------------- */

function renderTimer() {
  if (!recordingStartTime) {
    recordingTimer.textContent = "0:00";
    return;
  }

  const elapsed = Math.max(0, Math.floor((Date.now() - recordingStartTime) / 1000));
  const minutes = Math.floor(elapsed / 60);
  const seconds = String(elapsed % 60).padStart(2, "0");
  recordingTimer.textContent = `${minutes}:${seconds}`;
}

function levelToWaveformHeight(level: number) {
  const normalized = Math.max(0, Math.min(1, level));
  return 5 + Math.round(Math.pow(normalized, 0.55) * 21);
}

function ensureWaveformBars() {
  if (waveform.children.length === WAVEFORM_BAR_COUNT) return;

  waveform.replaceChildren(
    ...Array.from({ length: WAVEFORM_BAR_COUNT }, () => {
      const bar = document.createElement("span");
      return bar;
    }),
  );
}

function renderWaveform() {
  ensureWaveformBars();
  waveform.querySelectorAll("span").forEach((bar, index) => {
    (bar as HTMLSpanElement).style.height = `${levelToWaveformHeight(waveformLevels[index] ?? 0)}px`;
  });
}

function resetWaveform() {
  waveformLevels = Array.from({ length: WAVEFORM_BAR_COUNT }, () => 0);
  ensureWaveformBars();
  renderWaveform();
}

function pushAudioLevel(level: number) {
  const previous = waveformLevels[waveformLevels.length - 1] ?? 0;
  const smoothed = previous * 0.35 + Math.max(0, Math.min(1, level)) * 0.65;
  waveformLevels = [...waveformLevels.slice(1), smoothed];
  renderWaveform();
}

function decayWaveform() {
  waveformLevels = waveformLevels.map((level) => (level < 0.01 ? 0 : level * 0.82));
  renderWaveform();
}

function startRecordingUi() {
  window.clearInterval(recordingTimerInterval);
  window.clearInterval(waveformDecayInterval);
  recordingStartTime = Date.now();
  renderTimer();
  resetWaveform();
  recordingTimerInterval = window.setInterval(renderTimer, 250);
  waveformDecayInterval = window.setInterval(decayWaveform, 90);
}

function stopRecordingUi() {
  window.clearInterval(recordingTimerInterval);
  window.clearInterval(waveformDecayInterval);
  recordingTimerInterval = undefined;
  waveformDecayInterval = undefined;
  recordingStartTime = 0;
  resetWaveform();
}

/* ---------------------------------------------------------------------------
 * Visual state machine
 * ------------------------------------------------------------------------- */

function setVisualState(nextState: VisualState) {
  window.clearTimeout(completionTimer);

  if (nextState === "ready") {
    const nextCompletion = completionState(visualState);
    if (nextCompletion !== "ready") {
      setVisualState(nextCompletion);
      completionTimer = window.setTimeout(() => setVisualState("ready"), 1200);
      return;
    }
  }

  visualState = nextState;
  body.dataset.state = nextState;

  if (nextState === "recording") {
    startRecordingUi();
  } else {
    stopRecordingUi();
  }

  if (nextState !== "ready") {
    cancelExpand();
    cancelCollapse();
    expanded = false;
    menuOpen = false;
    clearTipNow();
    setHoveredElement(null);
  }

  render();
}

function setState(state: string) {
  setVisualState(normalizeState(state));
}

window.setTyprState = setState;

/* ---------------------------------------------------------------------------
 * Actions
 * ------------------------------------------------------------------------- */

async function toggleRecording() {
  try {
    await invoke("toggle_recording");
  } catch (error) {
    console.error("Failed to toggle recording", error);
  }
}

async function cancelRecordingCapture() {
  try {
    await invoke("cancel_recording");
  } catch (error) {
    console.error("Failed to cancel recording", error);
  }
}

async function openNotes() {
  try {
    await invoke("show_notes_window");
  } catch (error) {
    console.error("Failed to open notes", error);
  }
}

async function openSettings() {
  try {
    await invoke("show_settings_window");
  } catch (error) {
    console.error("Failed to open settings", error);
  }
}

function toggleMenu() {
  if (visualState !== "ready") return;
  cancelCollapse();
  setExpanded(true);
  setMenuOpen(!menuOpen);
}

async function pickTransform(transform: string) {
  if (!settings || settings.defaultTransform === transform) return;
  const transforms = configuredTransforms();
  if (!transforms.some((item) => item.id === transform)) return;
  await saveSettings({ ...settings, transforms, defaultTransform: transform });
}

/* ---------------------------------------------------------------------------
 * Wiring
 * ------------------------------------------------------------------------- */

collapsedHandle.addEventListener("click", () => {
  cancelExpand();
  setExpanded(true);
});

dictateButton.addEventListener("click", toggleRecording);
transformButton.addEventListener("click", toggleMenu);
transformCaret.addEventListener("click", toggleMenu);
notesButton.addEventListener("click", openNotes);
configureTransforms.addEventListener("click", openSettings);

autoApplyToggle.addEventListener("click", () => {
  if (!settings) return;
  void saveSettings({
    ...settings,
    transforms: configuredTransforms(),
    autoPolish: !settings.autoPolish,
  });
});

cancelRecording.addEventListener("click", cancelRecordingCapture);
stopRecording.addEventListener("click", toggleRecording);

// Close the transforms menu when clicking anywhere outside it.
document.addEventListener("mousedown", (event) => {
  if (!menuOpen) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest("#transform-popover, #transform-control")) return;
  setMenuOpen(false);
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  if (menuOpen) {
    setMenuOpen(false);
  } else if (expanded) {
    setExpanded(false);
  }
});

// Keyboard accessibility: surface tooltips on focus.
const focusTips: Array<[HTMLElement, TipTarget]> = [
  [dictateButton, "dictate"],
  [transformButton, "transform"],
  [transformCaret, "caret"],
  [notesButton, "notes"],
];

for (const [element, target] of focusTips) {
  element.addEventListener("focus", () => {
    if (visualState !== "ready") return;
    cancelCollapse();
    setExpanded(true);
    showTip(target);
  });
  element.addEventListener("blur", () => scheduleTipClear());
}

// Native mouse events: cover the focused-window case instantly and make the
// overlay fully previewable in a plain browser. The Rust watcher feeds the
// same pipeline when the window is unfocused.
document.addEventListener("mousemove", (event) => {
  handleCursor(event.clientX, event.clientY, true);
});
document.documentElement.addEventListener("mouseleave", () => {
  handleCursor(-1, -1, false);
});

listen<CursorPayload>("overlay-cursor", (event) => {
  const { x, y, inside } = event.payload;
  handleCursor(x, y, inside);
}).catch((error) => console.error("Failed to listen for cursor events", error));

listen<string>("recording-state", (event) => {
  setState(event.payload);
}).catch((error) => console.error("Failed to listen for recording state", error));

listen<AudioLevelPayload>("audio-level", (event) => {
  if (visualState !== "recording") return;
  pushAudioLevel(event.payload.level);
}).catch((error) => console.error("Failed to listen for audio levels", error));

listen<Settings>("settings-updated", (event) => {
  settings = event.payload;
  renderSettings();
}).catch((error) => console.error("Failed to listen for settings updates", error));

loadSettings().catch((error) => console.error("Failed to load settings", error));
invoke<string>("get_recording_state")
  .then(setState)
  .catch((error) => console.error("Failed to load recording state", error));

render();
