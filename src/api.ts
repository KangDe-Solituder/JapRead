import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  Annotation,
  Document,
  ParsePackage,
  Progress,
  Snapshot,
  SourceCatalog,
  ConnectionSettings,
  LlmConfig,
  DavConfig,
  AnalysisResult,
} from "./types";
export const desktop = isTauri();
async function request<T>(op: string, payload: unknown = {}): Promise<T> {
  if (desktop) return invoke<T>("request", { op, payload });
  const response = await fetch(`/api/${op}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!response.headers.get("content-type")?.includes("application/json"))
    throw new Error("本地服务未启动，请先运行 npm run server");
  const data = await response.json();
  if (!response.ok) throw new Error(data.error || "操作失败");
  return data as T;
}
export const api = {
  settings: () => request<ConnectionSettings>("settings_get"),
  saveLlmProfile: (
    id: string | null,
    name: string,
    config: LlmConfig,
    clearKey: boolean,
  ) =>
    request<ConnectionSettings>("llm_profile_save", {
      id,
      name,
      config,
      clearKey,
    }),
  activateLlmProfile: (id: string) =>
    request<ConnectionSettings>("llm_profile_activate", { id }),
  removeLlmProfile: (id: string) =>
    request<ConnectionSettings>("llm_profile_remove", { id }),
  saveSettings: (section: "llm" | "webdav", config: LlmConfig | DavConfig) =>
    request<ConnectionSettings>("settings_save", { section, config }),
  clearSettings: (section: "llm" | "webdav") =>
    request<ConnectionSettings>("settings_clear", { section }),
  testLlm: () => request<{ message: string }>("llm_test"),
  explainSelection: (
    documentId: string,
    chapterId: string,
    target: { sentenceId: string; start: number; end: number },
  ) =>
    request<AnalysisResult>("llm_selection", {
      documentId,
      chapterId,
      sentenceIds: [target.sentenceId],
      target: {
        sentenceId: target.sentenceId,
        start: target.start,
        end: target.end,
      },
    }),
  analyze: (documentId: string, chapterId: string, sentenceIds: string[]) =>
    request<AnalysisResult>("llm_analyze", {
      documentId,
      chapterId,
      sentenceIds,
    }),
  testWebdav: () => request<{ message: string }>("webdav_test"),
  uploadBackup: () =>
    request<{ filename: string; bytes: number }>("webdav_upload"),
  listBackups: () => request<string[]>("webdav_list"),
  restoreBackup: (filename: string) =>
    request<{ documents: number; annotations: number; conflicts: number }>(
      "webdav_restore",
      { filename },
    ),
  sourceProviders: () =>
    request<{ id: string; name: string; enabled: boolean }[]>(
      "source_providers",
    ),
  sourceCatalog: (input: {
    query?: string;
    authorId?: string;
    workQuery?: string;
    categories?: string[];
    orthographies?: string[];
    offset?: number;
    refresh?: boolean;
  }) =>
    request<SourceCatalog>("source_catalog", { provider: "aozora", ...input }),
  sourceImport: (workId: string) =>
    request<{ document: Document; existing: boolean }>("source_import", {
      provider: "aozora",
      workId,
    }),
  snapshot: () => request<Snapshot>("snapshot"),
  importDocument: (input: {
    title: string;
    author: string;
    kind: string;
    url: string;
    chapters: { title: string; text: string }[];
  }) => request<Document>("import_document", input),
  saveAnnotation: (a: Annotation) => request<Annotation>("save_annotation", a),
  setLearning: (id: string, learn: boolean) =>
    request<Annotation>("set_learning", { id, learn }),
  importAnnotations: (data: ParsePackage) =>
    request<{ added: number }>("import_annotations", data),
  saveProgress: (p: Progress) => request<Progress>("save_progress", p),
};
