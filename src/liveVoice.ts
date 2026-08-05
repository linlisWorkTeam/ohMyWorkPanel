/** Host-side Doubao-style voice UX via PanelLive proxy (no direct :8790). */

export const PANELLIVE_PROXY_BASE = "/api/extensions/panellive";

export type SttResponse = {
  text?: string;
  provider?: string;
  model?: string;
  fallbackReason?: string;
  error?: string;
};

export type TtsPlaybackResponse = {
  audioBase64?: string;
  audioContentType?: string;
  text?: string;
  truncated?: boolean;
  maxChars?: number;
  provider?: string;
  model?: string;
  error?: string;
};

export function secureMicAvailable(): boolean {
  return Boolean(
    typeof navigator !== "undefined"
      && navigator.mediaDevices
      && typeof navigator.mediaDevices.getUserMedia === "function",
  );
}

export function downsampleTo16k(input: Float32Array, inRate: number): Float32Array {
  if (inRate === 16000) return input;
  const ratio = inRate / 16000;
  const outLen = Math.floor(input.length / ratio);
  const out = new Float32Array(outLen);
  for (let i = 0; i < outLen; i++) out[i] = input[Math.floor(i * ratio)] ?? 0;
  return out;
}

/** Encode mono Float32 PCM as 16-bit WAV (aligned with WorkPanelLive live.html). */
export function encodeWav(float32Arrays: Float32Array[], sampleRate: number): Blob {
  let len = 0;
  for (const a of float32Arrays) len += a.length;
  const pcm = new Int16Array(len);
  let off = 0;
  for (const a of float32Arrays) {
    for (let i = 0; i < a.length; i++) {
      const s = Math.max(-1, Math.min(1, a[i] ?? 0));
      pcm[off++] = s < 0 ? s * 0x8000 : s * 0x7fff;
    }
  }
  const dataSize = pcm.length * 2;
  const buf = new ArrayBuffer(44 + dataSize);
  const v = new DataView(buf);
  const writeStr = (o: number, s: string) => {
    for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i));
  };
  writeStr(0, "RIFF");
  v.setUint32(4, 36 + dataSize, true);
  writeStr(8, "WAVE");
  writeStr(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, sampleRate, true);
  v.setUint32(28, sampleRate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  writeStr(36, "data");
  v.setUint32(40, dataSize, true);
  new Uint8Array(buf, 44).set(new Uint8Array(pcm.buffer));
  return new Blob([buf], { type: "audio/wav" });
}

export async function blobToBase64(blob: Blob): Promise<string> {
  const buf = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < buf.length; i += chunk) {
    binary += String.fromCharCode(...buf.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function buildSttBody(audioBase64: string, sessionId?: string | null): Record<string, string> {
  const body: Record<string, string> = {
    audioBase64,
    format: "wav",
    mimeType: "audio/wav",
  };
  if (sessionId) body.sessionId = sessionId;
  return body;
}

export function buildTtsPlaybackBody(text: string): { text: string; purpose: "playback"; sessionId: null } {
  return { text, purpose: "playback", sessionId: null };
}

export function combineComposerAndTranscript(composer: string, transcript: string): string {
  return [composer.trim(), transcript.trim()].filter(Boolean).join(" ");
}

type MicSession = {
  stream: MediaStream;
  ctx: AudioContext;
  processor: ScriptProcessorNode;
  buffers: Float32Array[];
  sampleRate: number;
};

let activeMic: MicSession | null = null;

export async function startHoldRecording(): Promise<void> {
  if (!secureMicAvailable()) {
    throw new Error("无法访问麦克风：需要 HTTPS 或 localhost（安全上下文）");
  }
  if (activeMic) return;
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const AC = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
  const ctx = new AC();
  const source = ctx.createMediaStreamSource(stream);
  const processor = ctx.createScriptProcessor(4096, 1, 1);
  const buffers: Float32Array[] = [];
  processor.onaudioprocess = (e) => {
    if (!activeMic) return;
    buffers.push(new Float32Array(e.inputBuffer.getChannelData(0)));
  };
  source.connect(processor);
  processor.connect(ctx.destination);
  activeMic = { stream, ctx, processor, buffers, sampleRate: ctx.sampleRate };
}

export async function stopHoldRecordingToWav(): Promise<Blob | null> {
  const mic = activeMic;
  activeMic = null;
  if (!mic) return null;
  try {
    mic.processor.disconnect();
  } catch {
    /* ignore */
  }
  try {
    await mic.ctx.close();
  } catch {
    /* ignore */
  }
  mic.stream.getTracks().forEach((t) => t.stop());
  if (mic.buffers.length === 0) return null;
  const down = mic.buffers.map((b) => downsampleTo16k(b, mic.sampleRate));
  return encodeWav(down, 16000);
}

export function cancelHoldRecording(): void {
  void stopHoldRecordingToWav();
}

export async function sttViaProxy(groupId: string, wav: Blob): Promise<SttResponse> {
  const audioBase64 = await blobToBase64(wav);
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "X-Linlis-Group-Id": groupId,
  };
  const post = async (path: string) => {
    const r = await fetch(`${PANELLIVE_PROXY_BASE}${path}`, {
      method: "POST",
      headers,
      body: JSON.stringify(buildSttBody(audioBase64)),
    });
    return (await r.json()) as SttResponse & { error?: string };
  };
  try {
    const stt = await post("/v1/stt");
    if (stt.error) {
      return post("/v1/stt/mock");
    }
    return stt;
  } catch {
    return post("/v1/stt/mock");
  }
}

export async function ttsPlaybackViaProxy(text: string): Promise<TtsPlaybackResponse> {
  const r = await fetch(`${PANELLIVE_PROXY_BASE}/v1/tts?format=json`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(buildTtsPlaybackBody(text)),
  });
  const j = (await r.json()) as TtsPlaybackResponse;
  if (j.error || !j.audioBase64) {
    const r2 = await fetch(`${PANELLIVE_PROXY_BASE}/v1/tts/mock?format=json`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(buildTtsPlaybackBody(text)),
    });
    return (await r2.json()) as TtsPlaybackResponse;
  }
  return j;
}

export function playAudioBase64(audioBase64: string, contentType: string): HTMLAudioElement {
  const mime = contentType || "audio/mpeg";
  const audio = new Audio(`data:${mime};base64,${audioBase64}`);
  void audio.play();
  return audio;
}
