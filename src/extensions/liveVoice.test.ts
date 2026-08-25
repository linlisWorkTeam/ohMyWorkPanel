import { describe, expect, it } from "vitest";
import {
  buildSttBody,
  buildTtsPlaybackBody,
  combineComposerAndTranscript,
  downsampleTo16k,
  encodeWav,
  secureMicAvailable,
} from "./liveVoice";

describe("liveVoice", () => {
  it("builds STT body with wav fields", () => {
    expect(buildSttBody("abc")).toEqual({
      audioBase64: "abc",
      format: "wav",
      mimeType: "audio/wav",
    });
    expect(buildSttBody("abc", "sid")).toMatchObject({ sessionId: "sid" });
  });

  it("builds TTS playback body with purpose=playback", () => {
    expect(buildTtsPlaybackBody("你好")).toEqual({
      text: "你好",
      purpose: "playback",
      sessionId: null,
    });
  });

  it("combines composer draft and transcript (mode B)", () => {
    expect(combineComposerAndTranscript("已有草稿", "语音内容")).toBe("已有草稿 语音内容");
    expect(combineComposerAndTranscript("", "仅语音")).toBe("仅语音");
    expect(combineComposerAndTranscript("  only  ", "")).toBe("only");
  });

  it("encodes WAV header for 16k mono PCM", async () => {
    const samples = [new Float32Array([0, 0.5, -0.5, 1])];
    const blob = encodeWav(samples, 16000);
    expect(blob.type).toBe("audio/wav");
    const buf = new Uint8Array(await blob.arrayBuffer());
    expect(String.fromCharCode(buf[0], buf[1], buf[2], buf[3])).toBe("RIFF");
    expect(String.fromCharCode(buf[8], buf[9], buf[10], buf[11])).toBe("WAVE");
    const view = new DataView(buf.buffer);
    expect(view.getUint32(24, true)).toBe(16000);
    expect(view.getUint16(22, true)).toBe(1);
  });

  it("downsamples to 16k", () => {
    const input = new Float32Array(32000);
    for (let i = 0; i < input.length; i++) input[i] = i / input.length;
    const out = downsampleTo16k(input, 32000);
    expect(out.length).toBe(16000);
  });

  it("reports mic availability from mediaDevices shape", () => {
    // jsdom may lack mediaDevices — function must not throw
    expect(typeof secureMicAvailable()).toBe("boolean");
  });
});
