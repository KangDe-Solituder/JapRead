export type Effort = "none" | "low" | "high";
export interface TaskOptions {
  effort: Effort;
  maxTokens: number;
  timeoutSeconds: number;
}
export interface ReadingProfile {
  name: string;
  context: number;
  readings: TaskOptions;
  analysis: TaskOptions;
}
export const standardProfile: ReadingProfile = {
  name: "日常",
  context: 1,
  readings: { effort: "low", maxTokens: 8000, timeoutSeconds: 90 },
  analysis: { effort: "low", maxTokens: 8000, timeoutSeconds: 90 },
};
export const deepProfile: ReadingProfile = {
  name: "深入",
  context: 3,
  readings: { effort: "high", maxTokens: 16000, timeoutSeconds: 180 },
  analysis: { effort: "high", maxTokens: 16000, timeoutSeconds: 180 },
};
export function validProfile(p: ReadingProfile) {
  return (
    Number.isInteger(p.context) &&
    p.context >= 0 &&
    p.context <= 5 &&
    [p.readings, p.analysis].every(
      (t) =>
        t &&
        ["none", "low", "high"].includes(t.effort) &&
        Number.isInteger(t.maxTokens) &&
        t.maxTokens >= 256 &&
        t.maxTokens <= 32768 &&
        Number.isInteger(t.timeoutSeconds) &&
        t.timeoutSeconds >= 15 &&
        t.timeoutSeconds <= 240,
    )
  );
}
export function cloneProfile(p: ReadingProfile): ReadingProfile {
  return JSON.parse(JSON.stringify(p));
}
