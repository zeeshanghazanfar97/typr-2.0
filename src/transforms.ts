export interface TextTransform {
  id: string;
  name: string;
  icon: string;
  systemPrompt: string;
}

export interface TransformStatusLabels {
  name: string;
  busy: string;
  done: string;
}

export interface TransformIconOption {
  id: string;
  label: string;
  svg: string;
}

const LEGACY_DEFAULT_POLISH_PROMPT =
  "You polish dictated speech-to-text transcripts. Return only the final corrected text. Preserve the speaker's meaning and do not add new facts. Apply explicit self-corrections, for example when the speaker says correction, I mean, rather, or sorry, keep the corrected wording and remove the abandoned wording. Remove filler words and accidental repetitions only when they are not meaningful. Restore punctuation and capitalization.";

const LEGACY_DEFAULT_PROMPT_ENGINEER_PROMPT =
  "You turn dictated speech into a clear, well-structured prompt for an AI assistant. Return only the final prompt text with no commentary. Preserve the speaker's intent, constraints, and details; do not invent requirements. Apply explicit self-corrections, for example when the speaker says correction, I mean, rather, or sorry, keep the corrected wording and remove the abandoned wording. Remove filler words and false starts. State the goal first, then context, then specific requirements as a short list when it helps, then the desired output format if one was mentioned. Restore punctuation and capitalization.";

export const DEFAULT_POLISH_PROMPT =
  "Rewrite the selected text to make it clearer, more concise, and easier to read. Remove filler words, repetition, awkward phrasing, and unnecessary complexity. Reorder sentences if needed for readability. Add light structure only when it improves clarity. Preserve the original meaning, intent, technical terms, URLs, markdown, and the writer's natural tone. Do not make it sound overly formal or AI-generated. Return only the rewritten text.";

export const DEFAULT_PROMPT_ENGINEER_PROMPT = `Transform the selected text into a clear, high-quality prompt for an AI assistant.

Structure the output with these sections:

Title:
A short, descriptive title for the prompt.

Role & stance:
Define what role the AI should take and how it should behave.

Task:
Clearly state what the AI needs to do.

Context:
Include all relevant background details, constraints, examples, and assumptions from the original text.

Output format:
Specify the desired structure, tone, length, and formatting of the answer.

Keep the user's original intent intact. Do not add unsupported requirements. Make the prompt specific, actionable, and easy for an AI model to follow. Return only the improved prompt.`;

export const NEW_TRANSFORM_PROMPT =
  "Transform the dictated text according to this transform's purpose. Return only the final transformed text. Preserve the user's intended meaning and do not add facts that were not dictated.";

export const DEFAULT_TRANSFORMS: TextTransform[] = [
  {
    id: "polish",
    name: "Polish",
    icon: "sparkles",
    systemPrompt: DEFAULT_POLISH_PROMPT,
  },
  {
    id: "promptEngineer",
    name: "Prompt Engineer",
    icon: "blocks",
    systemPrompt: DEFAULT_PROMPT_ENGINEER_PROMPT,
  },
];

export const TRANSFORM_ICONS: TransformIconOption[] = [
  {
    id: "sparkles",
    label: "Sparkles",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4.5 19.5 13 11" /><path d="m15.5 3.5 1 2.3 2.3 1-2.3 1-1 2.3-1-2.3-2.3-1 2.3-1 1-2.3Z" /><path d="m19.8 11.5.7 1.5 1.5.7-1.5.7-.7 1.5-.7-1.5-1.5-.7 1.5-.7.7-1.5Z" /></svg>',
  },
  {
    id: "blocks",
    label: "Blocks",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="6.5" height="6.5" rx="1.4" /><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.4" /><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.4" /><path d="M14 17h5.5M16.8 14.2v5.6" /></svg>',
  },
  {
    id: "pen",
    label: "Pen",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m4 20 4.2-1 10.6-10.6a2 2 0 0 0 0-2.8l-.4-.4a2 2 0 0 0-2.8 0L5 15.8 4 20Z" /><path d="m14.5 6.3 3.2 3.2" /></svg>',
  },
  {
    id: "quote",
    label: "Quote",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 7H5.8A2.8 2.8 0 0 0 3 9.8V12h5v5H3v-7.2A5.8 5.8 0 0 1 8.8 4H9" /><path d="M20 7h-2.2A2.8 2.8 0 0 0 15 9.8V12h5v5h-5v-7.2A5.8 5.8 0 0 1 20.8 4H21" /></svg>',
  },
  {
    id: "list",
    label: "List",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 6h11M8 12h11M8 18h11" /><path d="M4.5 6h.1M4.5 12h.1M4.5 18h.1" /></svg>',
  },
  {
    id: "mail",
    label: "Mail",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3.5" y="5.5" width="17" height="13" rx="2.2" /><path d="m4.5 7 7.5 6 7.5-6" /></svg>',
  },
  {
    id: "code",
    label: "Code",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8.5 8-4 4 4 4M15.5 8l4 4-4 4M13 5.5 11 18.5" /></svg>',
  },
  {
    id: "check",
    label: "Check",
    svg: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5 9.5 17 19 7" /><circle cx="12" cy="12" r="9" /></svg>',
  },
];

function normalizeSystemPrompt(transform: TextTransform, fallback: TextTransform) {
  const id = transform.id.trim();
  const systemPrompt = transform.systemPrompt.trim();

  if (!systemPrompt) return fallback.systemPrompt;
  if (id === "polish" && systemPrompt === LEGACY_DEFAULT_POLISH_PROMPT) {
    return DEFAULT_POLISH_PROMPT;
  }
  if (id === "promptEngineer" && systemPrompt === LEGACY_DEFAULT_PROMPT_ENGINEER_PROMPT) {
    return DEFAULT_PROMPT_ENGINEER_PROMPT;
  }

  return systemPrompt;
}

export function normalizeTransforms(transforms?: TextTransform[]) {
  const source = transforms?.length ? transforms : DEFAULT_TRANSFORMS;
  return source.map((transform, index) => {
    const fallback = DEFAULT_TRANSFORMS[index] ?? DEFAULT_TRANSFORMS[0];
    return {
      id: transform.id.trim() || fallback.id,
      name: transform.name.trim() || fallback.name,
      icon: getTransformIcon(transform.icon).id,
      systemPrompt: normalizeSystemPrompt(transform, fallback),
    };
  });
}

export function selectedTransform(
  transforms: TextTransform[] | undefined,
  selectedId: string | undefined,
) {
  const normalized = normalizeTransforms(transforms);
  return normalized.find((transform) => transform.id === selectedId) ?? normalized[0];
}

export function getTransformIcon(iconId: string | undefined) {
  return (
    TRANSFORM_ICONS.find((icon) => icon.id === iconId) ?? TRANSFORM_ICONS[0]
  );
}

export function getTransformIconSvg(iconId: string | undefined) {
  return getTransformIcon(iconId).svg;
}

export function statusLabelsForTransform(transform: TextTransform): TransformStatusLabels {
  if (transform.id === "polish") {
    return { name: transform.name, busy: "Polishing", done: "Polished" };
  }

  if (transform.id === "promptEngineer") {
    return { name: transform.name, busy: "Crafting prompt", done: "Prompt ready" };
  }

  return { name: transform.name, busy: "Transforming", done: `${transform.name} ready` };
}
