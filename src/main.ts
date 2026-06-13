import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  DEFAULT_DOCK_PREFERENCES,
  DOCK_INSET_OPTIONS,
  DOCK_POSITION_OPTIONS,
  DOCK_SHAPE_OPTIONS,
  DOCK_SIZE_OPTIONS,
  type DockColor,
  type DockPreferences,
  normalizeDockPreferences,
} from "./dock-preferences";
import {
  DEFAULT_TRANSFORMS,
  NEW_TRANSFORM_PROMPT,
  TRANSFORM_ICONS,
  type TextTransform,
  getTransformIconSvg,
  normalizeTransforms,
} from "./transforms";

interface Settings {
  microphone: string;
  engine: string;
  whisperModel: string;
  groqApiKey: string;
  transcriptModelMode: ModelMode;
  transcriptModelId: string;
  transformModelMode: ModelMode;
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

type ModelMode = "default" | "custom";

const DEFAULT_TRANSCRIPT_MODEL_ID = "whisper-large-v3-turbo";
const DEFAULT_TRANSFORM_MODEL_ID = "openai/gpt-oss-120b";

interface MicDevice {
  name: string;
  is_default: boolean;
}

interface DownloadProgress {
  downloaded: number;
  total: number;
  percent: number;
}

interface TranscriptTransformHistory {
  id: string;
  name: string;
  icon: string;
  model: string;
  systemPrompt: string;
  inputText: string;
  outputText: string;
  status: string;
  error: string;
}

interface TranscriptAudioHistory {
  fileName: string;
  mimeType: string;
  sizeBytes: number;
}

interface TranscriptHistoryEntry {
  id: string;
  createdAt: number;
  engine: string;
  microphone: string;
  transcriptModel: string;
  whisperModel: string;
  rawTranscript: string;
  cleanedTranscript: string;
  finalText: string;
  transform: TranscriptTransformHistory | null;
  pasteStatus: string;
  audio: TranscriptAudioHistory | null;
}

// DOM elements
const statusDot = document.getElementById("status-dot")!;
const statusText = document.getElementById("status-text")!;
const micSelect = document.getElementById("mic-select") as HTMLSelectElement;
const engineLocal = document.getElementById("engine-local")!;
const engineCloud = document.getElementById("engine-cloud")!;
const localSettings = document.getElementById("local-settings")!;
const cloudSettings = document.getElementById("cloud-settings")!;
const cloudTranscriptModelSettings = document.getElementById("cloud-transcript-model-settings")!;
const modelSelect = document.getElementById("model-select") as HTMLSelectElement;
const downloadBtn = document.getElementById("download-btn")!;
const downloadProgress = document.getElementById("download-progress")!;
const progressFill = document.getElementById("progress-fill")!;
const groqKey = document.getElementById("groq-key") as HTMLInputElement;
const transcriptModelDefault = document.getElementById("transcript-model-default") as HTMLButtonElement;
const transcriptModelCustom = document.getElementById("transcript-model-custom") as HTMLButtonElement;
const transcriptModelInput = document.getElementById("transcript-model-id") as HTMLInputElement;
const autoPolishToggle = document.getElementById("auto-polish-toggle") as HTMLButtonElement;
const defaultTransformSelect = document.getElementById("default-transform-select") as HTMLSelectElement;
const transformModelDefault = document.getElementById("transform-model-default") as HTMLButtonElement;
const transformModelCustom = document.getElementById("transform-model-custom") as HTMLButtonElement;
const transformModelInput = document.getElementById("transform-model-id") as HTMLInputElement;
const addTransformBtn = document.getElementById("add-transform-btn") as HTMLButtonElement;
const transformCount = document.getElementById("transform-count")!;
const transformList = document.getElementById("transform-list")!;
const dockSizeSelect = document.getElementById("dock-size-select") as HTMLSelectElement;
const dockShapeSelect = document.getElementById("dock-shape-select") as HTMLSelectElement;
const dockPositionSelect = document.getElementById("dock-position-select") as HTMLSelectElement;
const dockInsetSelect = document.getElementById("dock-inset-select") as HTMLSelectElement;
const dockColorOptions = Array.from(
  document.querySelectorAll<HTMLButtonElement>(".color-swatch-button"),
);
const modeToggle = document.getElementById("mode-toggle")!;
const modePtt = document.getElementById("mode-ptt")!;
const hotkeyText = document.getElementById("hotkey-text")!;
const hotkeyHint = document.getElementById("hotkey-hint")!;
const hotkeyChangeBtn = document.getElementById("hotkey-change-btn") as HTMLButtonElement;
const hotkeyClearBtn = document.getElementById("hotkey-clear-btn") as HTMLButtonElement;
const historyRefreshBtn = document.getElementById("history-refresh-btn") as HTMLButtonElement;
const historyClearBtn = document.getElementById("history-clear-btn") as HTMLButtonElement;
const historySummary = document.getElementById("history-summary")!;
const historyEmpty = document.getElementById("history-empty")!;
const historyList = document.getElementById("history-list")!;

// Section navigation
const navItems = document.querySelectorAll(".nav-item");
const sections = document.querySelectorAll(".content-section");

navItems.forEach((item) => {
  item.addEventListener("click", () => {
    const target = item.getAttribute("data-section");
    navItems.forEach((n) => n.classList.remove("active"));
    sections.forEach((s) => s.classList.remove("active"));
    item.classList.add("active");
    document.getElementById(`section-${target}`)?.classList.add("active");
  });
});

// Window drag — titlebar and sidebar empty space
const titlebar = document.getElementById("titlebar")!;
const sidebar = document.getElementById("sidebar")!;
let appWindow: ReturnType<typeof getCurrentWindow> | null = null;
const tauriInternals = (
  window as Window & { __TAURI_INTERNALS__?: { metadata?: unknown } }
).__TAURI_INTERNALS__;

if (tauriInternals?.metadata) {
  try {
    appWindow = getCurrentWindow();
  } catch (error) {
    console.warn("Tauri window APIs are unavailable in this preview", error);
  }
}

titlebar.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest("button, select, input, a, .nav-item")) return;
  appWindow?.startDragging();
});

sidebar.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest("button, select, input, a, .nav-item")) return;
  appWindow?.startDragging();
});

let currentSettings: Settings;
let isCapturingHotkey = false;
let currentHistory: TranscriptHistoryEntry[] = [];
const historyAudioUrls = new Map<string, string>();

function normalizeModelMode(mode: string | undefined): ModelMode {
  return mode === "custom" ? "custom" : "default";
}

function ensureModelSettings() {
  currentSettings.transcriptModelMode = normalizeModelMode(currentSettings.transcriptModelMode);
  currentSettings.transformModelMode = normalizeModelMode(currentSettings.transformModelMode);
  currentSettings.transcriptModelId = (currentSettings.transcriptModelId ?? "").trim();
  currentSettings.transformModelId = (currentSettings.transformModelId ?? "").trim();
}

function renderModelMode(
  mode: ModelMode,
  defaultButton: HTMLButtonElement,
  customButton: HTMLButtonElement,
  input: HTMLInputElement,
) {
  const isCustom = mode === "custom";
  defaultButton.classList.toggle("active", !isCustom);
  customButton.classList.toggle("active", isCustom);
  input.classList.toggle("hidden", !isCustom);
  input.disabled = !isCustom;
}

function renderModelSettings() {
  ensureModelSettings();
  transcriptModelInput.value = currentSettings.transcriptModelId;
  transformModelInput.value = currentSettings.transformModelId;
  transcriptModelInput.placeholder = DEFAULT_TRANSCRIPT_MODEL_ID;
  transformModelInput.placeholder = DEFAULT_TRANSFORM_MODEL_ID;

  renderModelMode(
    currentSettings.transcriptModelMode,
    transcriptModelDefault,
    transcriptModelCustom,
    transcriptModelInput,
  );
  renderModelMode(
    currentSettings.transformModelMode,
    transformModelDefault,
    transformModelCustom,
    transformModelInput,
  );
}

function setTranscriptModelMode(mode: ModelMode) {
  currentSettings.transcriptModelMode = mode;
  currentSettings.transcriptModelId = transcriptModelInput.value.trim();
  renderModelSettings();
  saveSettings();
  if (mode === "custom") transcriptModelInput.focus();
}

function setTransformModelMode(mode: ModelMode) {
  currentSettings.transformModelMode = mode;
  currentSettings.transformModelId = transformModelInput.value.trim();
  renderModelSettings();
  saveSettings();
  if (mode === "custom") transformModelInput.focus();
}

function formatHotkeyForDisplay(hotkey: string) {
  if (!hotkey.trim()) return "Not set";

  return hotkey
    .split("+")
    .map((part) => {
      const token = part.trim();
      if (/^Key[A-Z]$/.test(token)) return token.replace("Key", "");
      if (/^Digit[0-9]$/.test(token)) return token.replace("Digit", "");
      if (token === "Command") return "Cmd";
      if (token === "CmdOrCtrl") return "Cmd";
      if (token === "Alt") return "Option";
      return token;
    })
    .join(" + ");
}

function renderHotkey() {
  hotkeyText.textContent = isCapturingHotkey
    ? "Press shortcut"
    : formatHotkeyForDisplay(currentSettings.hotkey);
  hotkeyText.classList.toggle("empty", !currentSettings.hotkey.trim());
  hotkeyText.classList.toggle("capturing", isCapturingHotkey);
  hotkeyChangeBtn.textContent = isCapturingHotkey ? "Cancel" : "Change";
  hotkeyClearBtn.disabled = isCapturingHotkey || !currentSettings.hotkey.trim();

  if (isCapturingHotkey) {
    hotkeyHint.textContent = "Press modifiers, or modifiers plus a key. Esc cancels";
  } else if (!currentSettings.hotkey.trim()) {
    hotkeyHint.textContent = "No global shortcut is registered";
  } else {
    hotkeyHint.textContent = "Global keyboard shortcut to trigger recording";
  }
}

function populateSelect<T extends string>(
  select: HTMLSelectElement,
  options: Array<{ value: T; label: string }>,
) {
  select.replaceChildren(
    ...options.map((optionConfig) => {
      const option = document.createElement("option");
      option.value = optionConfig.value;
      option.textContent = optionConfig.label;
      return option;
    }),
  );
}

function populateDockControls() {
  populateSelect(dockSizeSelect, DOCK_SIZE_OPTIONS);
  populateSelect(dockShapeSelect, DOCK_SHAPE_OPTIONS);
  populateSelect(dockPositionSelect, DOCK_POSITION_OPTIONS);
  populateSelect(
    dockInsetSelect,
    DOCK_INSET_OPTIONS.map((option) => ({
      value: String(option.value),
      label: option.label,
    })),
  );
  dockSizeSelect.value = DEFAULT_DOCK_PREFERENCES.dockSize;
  dockShapeSelect.value = DEFAULT_DOCK_PREFERENCES.dockShape;
  dockPositionSelect.value = DEFAULT_DOCK_PREFERENCES.dockPosition;
  dockInsetSelect.value = String(DEFAULT_DOCK_PREFERENCES.dockInset);
}

function ensureDockSettings() {
  const preferences = normalizeDockPreferences(currentSettings);
  currentSettings.dockSize = preferences.dockSize;
  currentSettings.dockShape = preferences.dockShape;
  currentSettings.dockColor = preferences.dockColor;
  currentSettings.dockPosition = preferences.dockPosition;
  currentSettings.dockInset = preferences.dockInset;
}

function renderDockSettings() {
  ensureDockSettings();
  dockSizeSelect.value = currentSettings.dockSize;
  dockShapeSelect.value = currentSettings.dockShape;
  dockPositionSelect.value = currentSettings.dockPosition;
  dockInsetSelect.value = String(currentSettings.dockInset);

  for (const option of dockColorOptions) {
    const selected = option.dataset.dockColor === currentSettings.dockColor;
    option.classList.toggle("active", selected);
    option.setAttribute("aria-checked", String(selected));
    option.title = option.getAttribute("aria-label") ?? "";
  }
}

function saveDockPreference(patch: Partial<DockPreferences>) {
  Object.assign(currentSettings, patch);
  renderDockSettings();
  saveSettings();
}

function ensureTransformSettings() {
  currentSettings.transforms = normalizeTransforms(currentSettings.transforms);
  if (
    !currentSettings.transforms.some(
      (transform) => transform.id === currentSettings.defaultTransform,
    )
  ) {
    currentSettings.defaultTransform = currentSettings.transforms[0]?.id ?? DEFAULT_TRANSFORMS[0].id;
  }
}

function createTransformId() {
  return `custom-${Date.now().toString(36)}-${Math.round(Math.random() * 9999)}`;
}

function createIconPreview(iconId: string) {
  const preview = document.createElement("span");
  preview.className = "transform-icon-preview";
  preview.innerHTML = getTransformIconSvg(iconId);
  return preview;
}

function syncDefaultTransformSelect() {
  defaultTransformSelect.replaceChildren(
    ...currentSettings.transforms.map((transform) => {
      const option = document.createElement("option");
      option.value = transform.id;
      option.textContent = transform.name;
      return option;
    }),
  );
  defaultTransformSelect.value = currentSettings.defaultTransform;
}

function updateTransform(id: string, patch: Partial<TextTransform>) {
  currentSettings.transforms = currentSettings.transforms.map((transform) =>
    transform.id === id ? { ...transform, ...patch } : transform,
  );
}

function saveAndReport(errorContext: string) {
  saveSettings().catch((error) => {
    console.error(errorContext, error);
  });
}

function renderTransformEditor(transform: TextTransform) {
  const editor = document.createElement("article");
  editor.className = "transform-editor";
  editor.dataset.transformId = transform.id;

  const header = document.createElement("div");
  header.className = "transform-editor-header";

  const identity = document.createElement("div");
  identity.className = "transform-editor-identity";
  const iconPreview = createIconPreview(transform.icon);

  const nameInput = document.createElement("input");
  nameInput.className = "transform-name-input";
  nameInput.type = "text";
  nameInput.value = transform.name;
  nameInput.placeholder = "Transform name";
  nameInput.setAttribute("aria-label", "Transform name");

  identity.append(iconPreview, nameInput);

  const actions = document.createElement("div");
  actions.className = "transform-editor-actions";

  const iconSelect = document.createElement("select");
  iconSelect.className = "transform-icon-select";
  iconSelect.setAttribute("aria-label", "Transform icon");
  iconSelect.replaceChildren(
    ...TRANSFORM_ICONS.map((icon) => {
      const option = document.createElement("option");
      option.value = icon.id;
      option.textContent = icon.label;
      return option;
    }),
  );
  iconSelect.value = transform.icon;

  const defaultButton = document.createElement("button");
  defaultButton.className = "btn-secondary transform-default-btn";
  defaultButton.type = "button";
  defaultButton.textContent =
    currentSettings.defaultTransform === transform.id ? "Default" : "Make Default";
  defaultButton.disabled = currentSettings.defaultTransform === transform.id;

  const deleteButton = document.createElement("button");
  deleteButton.className = "btn-secondary transform-delete-btn";
  deleteButton.type = "button";
  deleteButton.textContent = "Delete";
  deleteButton.disabled = currentSettings.transforms.length <= 1;

  actions.append(iconSelect, defaultButton, deleteButton);
  header.append(identity, actions);

  const promptLabel = document.createElement("label");
  promptLabel.className = "transform-prompt-field";

  const promptText = document.createElement("span");
  promptText.className = "label-hint";
  promptText.textContent = "System prompt";

  const promptInput = document.createElement("textarea");
  promptInput.value = transform.systemPrompt;
  promptInput.rows = 5;
  promptInput.spellcheck = true;
  promptInput.placeholder = "Tell the LLM how to transform dictated text";

  promptLabel.append(promptText, promptInput);
  editor.append(header, promptLabel);

  nameInput.addEventListener("input", () => {
    updateTransform(transform.id, { name: nameInput.value });
  });

  nameInput.addEventListener("change", () => {
    const name = nameInput.value.trim() || "Untitled Transform";
    updateTransform(transform.id, { name });
    renderTransforms();
    saveAndReport("Failed to save transform name:");
  });

  iconSelect.addEventListener("change", () => {
    updateTransform(transform.id, { icon: iconSelect.value });
    iconPreview.innerHTML = getTransformIconSvg(iconSelect.value);
    saveAndReport("Failed to save transform icon:");
  });

  defaultButton.addEventListener("click", () => {
    currentSettings.defaultTransform = transform.id;
    renderTransforms();
    saveAndReport("Failed to save default transform:");
  });

  deleteButton.addEventListener("click", () => {
    if (currentSettings.transforms.length <= 1) return;
    currentSettings.transforms = currentSettings.transforms.filter(
      (item) => item.id !== transform.id,
    );
    if (currentSettings.defaultTransform === transform.id) {
      currentSettings.defaultTransform = currentSettings.transforms[0].id;
    }
    renderTransforms();
    saveAndReport("Failed to delete transform:");
  });

  promptInput.addEventListener("input", () => {
    updateTransform(transform.id, { systemPrompt: promptInput.value });
  });

  promptInput.addEventListener("change", () => {
    updateTransform(transform.id, { systemPrompt: promptInput.value });
    saveAndReport("Failed to save transform prompt:");
  });

  return editor;
}

function renderTransforms() {
  ensureTransformSettings();
  syncDefaultTransformSelect();
  transformCount.textContent = `${currentSettings.transforms.length} configured`;
  transformList.replaceChildren(
    ...currentSettings.transforms.map((transform) => renderTransformEditor(transform)),
  );
}

function formatHistoryTime(createdAt: number) {
  if (!Number.isFinite(createdAt) || createdAt <= 0) return "Unknown time";

  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(createdAt));
}

function formatFileSize(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Unknown size";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function revokeHistoryAudioUrls() {
  historyAudioUrls.forEach((url) => URL.revokeObjectURL(url));
  historyAudioUrls.clear();
}

function labelFromValue(value: string) {
  return value
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function createHistoryPill(label: string, tone?: "good" | "warn" | "muted") {
  const pill = document.createElement("span");
  pill.className = "history-pill";
  if (tone) pill.dataset.tone = tone;
  pill.textContent = label;
  return pill;
}

function createHistoryTextBlock(label: string, value: string) {
  const block = document.createElement("div");
  block.className = "history-text-block";

  const blockLabel = document.createElement("span");
  blockLabel.className = "history-text-label";
  blockLabel.textContent = label;

  const text = document.createElement("pre");
  text.textContent = value.trim() || "Empty";

  block.append(blockLabel, text);
  return block;
}

function renderHistoryAudio(entry: TranscriptHistoryEntry) {
  const wrapper = document.createElement("div");
  wrapper.className = "history-audio";

  const label = document.createElement("div");
  label.className = "history-audio-label";
  const title = document.createElement("span");
  title.className = "history-text-label";
  title.textContent = "Recording";
  const detail = document.createElement("span");
  detail.className = "label-hint";
  detail.textContent = entry.audio ? formatFileSize(entry.audio.sizeBytes) : "No audio saved";
  label.append(title, detail);

  const controls = document.createElement("div");
  controls.className = "history-audio-controls";

  if (!entry.audio) {
    controls.append(createHistoryPill("Unavailable", "muted"));
    wrapper.append(label, controls);
    return wrapper;
  }

  const loadButton = document.createElement("button");
  loadButton.className = "btn-secondary history-audio-load";
  loadButton.type = "button";
  loadButton.textContent = "Load";

  const audio = document.createElement("audio");
  audio.className = "history-audio-player hidden";
  audio.controls = true;
  audio.preload = "metadata";

  loadButton.addEventListener("click", async () => {
    loadButton.disabled = true;
    loadButton.textContent = "Loading";

    try {
      const existingUrl = historyAudioUrls.get(entry.id);
      const url =
        existingUrl ??
        URL.createObjectURL(
          new Blob([new Uint8Array(await invoke<number[]>("get_history_audio", { id: entry.id }))], {
            type: entry.audio?.mimeType || "audio/wav",
          }),
        );

      historyAudioUrls.set(entry.id, url);
      audio.src = url;
      audio.classList.remove("hidden");
      loadButton.remove();
      await audio.play().catch(() => undefined);
    } catch (error) {
      console.error("Failed to load history audio:", error);
      loadButton.disabled = false;
      loadButton.textContent = "Retry";
    }
  });

  controls.append(loadButton, audio);
  wrapper.append(label, controls);
  return wrapper;
}

function renderHistoryTransform(transform: TranscriptTransformHistory) {
  const wrapper = document.createElement("div");
  wrapper.className = "history-transform";

  const header = document.createElement("div");
  header.className = "history-transform-header";

  const identity = document.createElement("div");
  identity.className = "history-transform-identity";
  const icon = createIconPreview(transform.icon);
  const text = document.createElement("div");

  const name = document.createElement("span");
  name.className = "label-text";
  name.textContent = transform.name || "Transform";

  const model = document.createElement("span");
  model.className = "label-hint";
  model.textContent = transform.model || "Default model";

  text.append(name, model);
  identity.append(icon, text);

  const statusTone = transform.status === "applied" ? "good" : "warn";
  header.append(identity, createHistoryPill(labelFromValue(transform.status), statusTone));
  wrapper.append(header);

  const grid = document.createElement("div");
  grid.className = "history-text-grid";
  grid.append(
    createHistoryTextBlock("Transform input", transform.inputText),
    createHistoryTextBlock("Transform output", transform.outputText || transform.error),
  );
  wrapper.append(grid);

  const promptDetails = document.createElement("details");
  promptDetails.className = "history-details";
  const summary = document.createElement("summary");
  summary.textContent = "System prompt";
  const prompt = document.createElement("pre");
  prompt.textContent = transform.systemPrompt || "Empty";
  promptDetails.append(summary, prompt);
  wrapper.append(promptDetails);

  return wrapper;
}

function renderHistoryEntry(entry: TranscriptHistoryEntry) {
  const item = document.createElement("article");
  item.className = "history-entry";

  const header = document.createElement("div");
  header.className = "history-entry-header";

  const title = document.createElement("div");
  title.className = "history-entry-title";
  const time = document.createElement("span");
  time.className = "label-text";
  time.textContent = formatHistoryTime(entry.createdAt);
  const meta = document.createElement("span");
  meta.className = "label-hint";
  meta.textContent = `${labelFromValue(entry.engine)} · ${entry.microphone || "Default mic"}`;
  title.append(time, meta);

  const pills = document.createElement("div");
  pills.className = "history-entry-pills";
  const pasteTone =
    entry.pasteStatus === "pasted"
      ? "good"
      : entry.pasteStatus === "failed"
        ? "warn"
        : "muted";
  pills.append(
    createHistoryPill(`Paste ${labelFromValue(entry.pasteStatus)}`, pasteTone),
    createHistoryPill(entry.transform ? "Transform" : "Raw", entry.transform ? "good" : "muted"),
  );
  header.append(title, pills);

  const modelMeta = document.createElement("div");
  modelMeta.className = "history-model-meta";
  modelMeta.append(
    createHistoryPill(
      entry.engine === "cloud"
        ? `Cloud ${entry.transcriptModel || "default"}`
        : `Local ${entry.whisperModel || "small"}`,
      "muted",
    ),
  );
  if (entry.transform?.model) {
    modelMeta.append(createHistoryPill(`Transform ${entry.transform.model}`, "muted"));
  }

  const grid = document.createElement("div");
  grid.className = "history-text-grid";
  grid.append(
    createHistoryTextBlock("Raw transcript", entry.rawTranscript),
    createHistoryTextBlock("Cleaned transcript", entry.cleanedTranscript),
    createHistoryTextBlock("Final text", entry.finalText),
  );

  item.append(header, modelMeta, renderHistoryAudio(entry), grid);
  if (entry.transform) {
    item.append(renderHistoryTransform(entry.transform));
  }

  return item;
}

function renderHistory(history: TranscriptHistoryEntry[]) {
  const count = history.length;
  revokeHistoryAudioUrls();
  historySummary.textContent = `${count} ${count === 1 ? "entry" : "entries"}`;
  historyClearBtn.disabled = count === 0;
  historyEmpty.classList.toggle("hidden", count !== 0);
  historyList.replaceChildren(...history.map(renderHistoryEntry));
}

async function loadHistory() {
  historyRefreshBtn.disabled = true;
  historySummary.textContent = "Loading";

  try {
    currentHistory = await invoke<TranscriptHistoryEntry[]>("get_history");
    renderHistory(currentHistory);
  } catch (error) {
    currentHistory = [];
    revokeHistoryAudioUrls();
    historySummary.textContent = "Unavailable";
    historyClearBtn.disabled = true;
    historyEmpty.classList.remove("hidden");
    historyList.replaceChildren();
    console.warn("History is unavailable in this preview", error);
  } finally {
    historyRefreshBtn.disabled = false;
  }
}

function applySettingsToControls() {
  ensureDockSettings();
  ensureModelSettings();
  ensureTransformSettings();
  micSelect.value = currentSettings.microphone;
  setEngine(currentSettings.engine);
  modelSelect.value = currentSettings.whisperModel;
  groqKey.value = currentSettings.groqApiKey;
  setAutoPolish(currentSettings.autoPolish);
  setRecordingMode(currentSettings.recordingMode);
  renderModelSettings();
  renderDockSettings();
  renderTransforms();
  renderHotkey();
}

async function loadSettings() {
  currentSettings = await invoke<Settings>("get_settings");

  // Populate mic dropdown
  const mics = await invoke<MicDevice[]>("list_microphones");
  micSelect.innerHTML = "";
  mics.forEach((mic) => {
    const option = document.createElement("option");
    option.value = mic.name;
    option.textContent = mic.name + (mic.is_default ? " (default)" : "");
    micSelect.appendChild(option);
  });
  applySettingsToControls();
  await checkModelStatus();
}

function setEngine(engine: string) {
  currentSettings.engine = engine;
  engineLocal.classList.toggle("active", engine === "local");
  engineCloud.classList.toggle("active", engine === "cloud");
  localSettings.classList.toggle("hidden", engine !== "local");
  cloudSettings.classList.toggle("hidden", engine !== "cloud");
  cloudTranscriptModelSettings.classList.toggle("hidden", engine !== "cloud");
}

function setRecordingMode(mode: string) {
  currentSettings.recordingMode = mode;
  modeToggle.classList.toggle("active", mode === "toggle");
  modePtt.classList.toggle("active", mode === "push-to-talk");
}

function setAutoPolish(enabled: boolean) {
  currentSettings.autoPolish = enabled;
  autoPolishToggle.classList.toggle("active", enabled);
  autoPolishToggle.setAttribute("aria-checked", String(enabled));
}

async function checkModelStatus() {
  const downloaded = await invoke<boolean>("check_model_downloaded", {
    modelSize: modelSelect.value,
  });
  downloadBtn.textContent = downloaded ? "\u2713" : "Download";
  (downloadBtn as HTMLButtonElement).disabled = downloaded;
}

async function saveSettings() {
  ensureDockSettings();
  ensureModelSettings();
  ensureTransformSettings();
  currentSettings.microphone = micSelect.value;
  currentSettings.whisperModel = modelSelect.value;
  currentSettings.groqApiKey = groqKey.value;
  currentSettings.transcriptModelId = transcriptModelInput.value.trim();
  currentSettings.transformModelId = transformModelInput.value.trim();
  await invoke("save_settings", { settings: currentSettings });
}

function keyFromKeyboardEvent(event: KeyboardEvent) {
  if (["Shift", "Control", "Alt", "Meta"].includes(event.key)) return null;

  const code = event.code || event.key;
  const aliases: Record<string, string> = {
    " ": "Space",
    Esc: "Escape",
    Return: "Enter",
  };

  return aliases[code] ?? aliases[event.key] ?? code;
}

function modifiersFromKeyboardEvent(event: KeyboardEvent) {
  const modifiers: string[] = [];
  if (event.metaKey) modifiers.push("Command");
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.altKey) modifiers.push("Option");
  if (event.shiftKey) modifiers.push("Shift");

  return modifiers;
}

function hotkeyFromKeyboardEvent(event: KeyboardEvent) {
  const modifiers = modifiersFromKeyboardEvent(event);
  const key = keyFromKeyboardEvent(event);

  if (!key) {
    if (modifiers.length >= 2) return modifiers.join("+");
    hotkeyHint.textContent = "Press another modifier or add a key";
    return null;
  }

  if (modifiers.length === 0) {
    hotkeyHint.textContent = "Use at least one modifier";
    return null;
  }

  return [...modifiers, key].join("+");
}

function stopHotkeyCapture() {
  isCapturingHotkey = false;
  renderHotkey();
}

async function persistHotkey(nextHotkey: string) {
  const previousHotkey = currentSettings.hotkey;
  currentSettings.hotkey = nextHotkey;
  renderHotkey();

  try {
    await saveSettings();
  } catch (error) {
    currentSettings.hotkey = previousHotkey;
    renderHotkey();
    hotkeyHint.textContent =
      error instanceof Error ? error.message : String(error);
    console.error("Failed to save hotkey:", error);
  }
}

async function handleHotkeyCapture(event: KeyboardEvent) {
  if (!isCapturingHotkey) return;

  event.preventDefault();
  event.stopPropagation();

  if (event.key === "Escape") {
    stopHotkeyCapture();
    return;
  }

  const nextHotkey = hotkeyFromKeyboardEvent(event);
  if (!nextHotkey) return;

  stopHotkeyCapture();
  await persistHotkey(nextHotkey);
}

// Event listeners
engineLocal.addEventListener("click", () => {
  setEngine("local");
  saveSettings();
});

engineCloud.addEventListener("click", () => {
  setEngine("cloud");
  saveSettings();
});

micSelect.addEventListener("change", () => saveSettings());

modelSelect.addEventListener("change", async () => {
  await checkModelStatus();
  saveSettings();
});

downloadBtn.addEventListener("click", async () => {
  (downloadBtn as HTMLButtonElement).disabled = true;
  downloadProgress.classList.remove("hidden");
  progressFill.style.width = "0%";

  try {
    await invoke("download_model", { modelSize: modelSelect.value });
    downloadBtn.textContent = "\u2713";
  } catch (e) {
    downloadBtn.textContent = "Retry";
    (downloadBtn as HTMLButtonElement).disabled = false;
    console.error("Download failed:", e);
  }
  downloadProgress.classList.add("hidden");
});

groqKey.addEventListener("change", () => saveSettings());

transcriptModelDefault.addEventListener("click", () => setTranscriptModelMode("default"));

transcriptModelCustom.addEventListener("click", () => setTranscriptModelMode("custom"));

transcriptModelInput.addEventListener("change", () => {
  currentSettings.transcriptModelId = transcriptModelInput.value.trim();
  renderModelSettings();
  saveSettings();
});

autoPolishToggle.addEventListener("click", () => {
  setAutoPolish(!currentSettings.autoPolish);
  saveSettings();
});

defaultTransformSelect.addEventListener("change", () => {
  currentSettings.defaultTransform = defaultTransformSelect.value;
  renderTransforms();
  saveSettings();
});

transformModelDefault.addEventListener("click", () => setTransformModelMode("default"));

transformModelCustom.addEventListener("click", () => setTransformModelMode("custom"));

transformModelInput.addEventListener("change", () => {
  currentSettings.transformModelId = transformModelInput.value.trim();
  renderModelSettings();
  saveSettings();
});

dockSizeSelect.addEventListener("change", () => {
  saveDockPreference({ dockSize: dockSizeSelect.value as DockPreferences["dockSize"] });
});

dockShapeSelect.addEventListener("change", () => {
  saveDockPreference({ dockShape: dockShapeSelect.value as DockPreferences["dockShape"] });
});

dockPositionSelect.addEventListener("change", () => {
  saveDockPreference({
    dockPosition: dockPositionSelect.value as DockPreferences["dockPosition"],
  });
});

dockInsetSelect.addEventListener("change", () => {
  saveDockPreference({ dockInset: Number(dockInsetSelect.value) });
});

for (const option of dockColorOptions) {
  option.addEventListener("click", () => {
    const dockColor = option.dataset.dockColor as DockColor | undefined;
    if (!dockColor) return;
    saveDockPreference({ dockColor });
  });
}

addTransformBtn.addEventListener("click", () => {
  const transform: TextTransform = {
    id: createTransformId(),
    name: "New Transform",
    icon: "pen",
    systemPrompt: NEW_TRANSFORM_PROMPT,
  };

  currentSettings.transforms = [...currentSettings.transforms, transform];
  currentSettings.defaultTransform = transform.id;
  renderTransforms();
  saveSettings();

  const nameInput = transformList.querySelector<HTMLInputElement>(
    `[data-transform-id="${transform.id}"] .transform-name-input`,
  );
  nameInput?.focus();
  nameInput?.select();
});

modeToggle.addEventListener("click", () => {
  setRecordingMode("toggle");
  saveSettings();
});

modePtt.addEventListener("click", () => {
  setRecordingMode("push-to-talk");
  saveSettings();
});

hotkeyChangeBtn.addEventListener("click", () => {
  isCapturingHotkey = !isCapturingHotkey;
  renderHotkey();
});

hotkeyClearBtn.addEventListener("click", () => {
  stopHotkeyCapture();
  persistHotkey("");
});

historyRefreshBtn.addEventListener("click", () => {
  loadHistory();
});

historyClearBtn.addEventListener("click", async () => {
  if (!currentHistory.length) return;
  if (!window.confirm("Clear transcript history?")) return;

  historyClearBtn.disabled = true;
  try {
    await invoke("clear_history");
    await loadHistory();
  } catch (error) {
    historyClearBtn.disabled = false;
    console.error("Failed to clear history:", error);
  }
});

document.addEventListener("keydown", handleHotkeyCapture, true);

// Listen for recording state changes
listen<string>("recording-state", (event) => {
  const state = event.payload;
  statusDot.className = "";
  if (state === "Recording") {
    statusDot.classList.add("recording");
    statusText.textContent = "Recording...";
  } else if (state === "Transcribing") {
    statusDot.classList.add("transcribing");
    statusText.textContent = "Transcribing...";
  } else {
    statusDot.classList.add("ready");
    statusText.textContent = "Ready";
  }
}).catch((error) => console.error("Failed to listen for recording state", error));

// Listen for download progress
listen<DownloadProgress>("download-progress", (event) => {
  const { percent } = event.payload;
  progressFill.style.width = `${percent}%`;
}).catch((error) => console.error("Failed to listen for download progress", error));

listen<Settings>("settings-updated", async (event) => {
  currentSettings = event.payload;
  applySettingsToControls();
  await checkModelStatus();
}).catch((error) => console.error("Failed to listen for settings updates", error));

listen<void>("history-updated", () => {
  loadHistory();
}).catch((error) => console.error("Failed to listen for history updates", error));

// Initialize
populateDockControls();
loadSettings().catch((error) => {
  console.warn("Settings are unavailable in this preview", error);
});
loadHistory();
