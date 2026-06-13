import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const STORAGE_KEY = "typr.notes";
const SAVE_DEBOUNCE_MS = 300;
const STATUS_FLASH_MS = 1400;

const noteInput = document.getElementById("note-input") as HTMLTextAreaElement;
const closeButton = document.getElementById("close-notes") as HTMLButtonElement;
const dictateButton = document.getElementById("dictate-note") as HTMLButtonElement;
const dictateLabel = document.getElementById("dictate-note-label")!;
const noteStatus = document.getElementById("note-status")!;

let saveTimer: number | undefined;
let statusTimer: number | undefined;

noteInput.value = localStorage.getItem(STORAGE_KEY) ?? "";

function flashStatus(text: string) {
  window.clearTimeout(statusTimer);
  noteStatus.textContent = text;
  noteStatus.classList.add("visible");
  statusTimer = window.setTimeout(() => {
    noteStatus.classList.remove("visible");
  }, STATUS_FLASH_MS);
}

function persistNow() {
  window.clearTimeout(saveTimer);
  saveTimer = undefined;
  localStorage.setItem(STORAGE_KEY, noteInput.value);
}

noteInput.addEventListener("input", () => {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    persistNow();
    flashStatus("Saved");
  }, SAVE_DEBOUNCE_MS);
});

closeButton.addEventListener("click", async () => {
  persistNow();
  try {
    await invoke("hide_notes_window");
  } catch (error) {
    console.error("Failed to hide notes window", error);
  }
});

dictateButton.addEventListener("click", async () => {
  // Keep the caret in the note so the transcript pastes here.
  noteInput.focus();
  try {
    await invoke("toggle_recording");
  } catch (error) {
    console.error("Failed to toggle recording", error);
  }
});

function applyRecordingState(state: string) {
  const normalized = String(state).toLowerCase();
  if (normalized === "recording") {
    dictateButton.dataset.state = "recording";
    dictateLabel.textContent = "Listening… click to stop";
  } else if (normalized === "transcribing" || normalized === "polishing") {
    dictateButton.dataset.state = "transcribing";
    dictateLabel.textContent = "Transcribing…";
  } else if (normalized === "canceled" || normalized === "cancelled") {
    dictateButton.dataset.state = "ready";
    dictateLabel.textContent = "Dictate";
    flashStatus("Dictation cancelled");
  } else {
    dictateButton.dataset.state = "ready";
    dictateLabel.textContent = "Dictate";
  }
}

listen<string>("recording-state", (event) => {
  applyRecordingState(event.payload);
}).catch((error) => console.error("Failed to listen for recording state", error));

invoke<string>("get_recording_state")
  .then(applyRecordingState)
  .catch((error) => console.error("Failed to load recording state", error));

document.addEventListener("keydown", (event) => {
  if (event.key === "w" && (event.metaKey || event.ctrlKey)) {
    event.preventDefault();
    persistNow();
    invoke("hide_notes_window").catch((error) =>
      console.error("Failed to hide notes window", error),
    );
  }
});

window.addEventListener("focus", () => noteInput.focus());
window.addEventListener("blur", persistNow);

noteInput.focus();
