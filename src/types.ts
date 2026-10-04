export interface Sentence {
  id: string;
  text: string;
  paragraph: number;
  readings?: { start: number; end: number; reading: string }[];
}
export interface Chapter {
  id: string;
  title: string;
  sentences: Sentence[];
}
export interface Document {
  id: string;
  title: string;
  author: string;
  kind: string;
  url: string;
  version: number;
  chapters: Chapter[];
  source?: {
    key: string;
    provider: string;
    workId: string;
    notice: string;
    credits?: string;
  };
}
export interface Annotation {
  id: string;
  documentId: string;
  version: number;
  sentenceId: string;
  start: number;
  end: number;
  quote: string;
  kind: string;
  reading: string;
  meaning: string;
  explanation: string;
  learn: boolean;
}
export interface Progress {
  documentId: string;
  chapterId: string;
  sentenceId: string;
}
export interface Snapshot {
  documents: Document[];
  annotations: Annotation[];
  progress: Progress[];
}
export interface ParsePackage {
  schemaVersion: number;
  documentId: string;
  version: number;
  annotations: Annotation[];
}
export interface LlmConfig {
  endpoint: string;
  model: string;
  protocol: "chat" | "responses";
  apiKey: string;
  timeoutSeconds: number;
  maxTokens: number;
  structured: boolean;
  extraBody: string;
}
export interface DavConfig {
  endpoint: string;
  username: string;
  password: string;
  remotePath: string;
}
export interface ConnectionSettings {
  llmProfiles: {
    id: string;
    name: string;
    config: LlmConfig;
    hasApiKey: boolean;
  }[];
  activeLlmId: string | null;
  llm: LlmConfig;
  webdav: DavConfig;
  hasApiKey: boolean;
  hasDavPassword: boolean;
  secureStorage: boolean;
}
export interface AnalysisResult {
  package: ParsePackage;
  warnings?: string[];
  elapsedMs: number;
  usage: { inputTokens: number | null; outputTokens: number | null };
}

export interface SourceAuthor {
  id: string;
  name: string;
  reading: string;
  count: number;
}
export interface SourceWork {
  id: string;
  title: string;
  subtitle: string;
  authors: string[];
  category: string;
  ndc: string;
  orthography: string;
  url: string;
  htmlUrl: string;
  copyright: string;
}
export interface SourceCatalog {
  authors: SourceAuthor[];
  authorTotal: number;
  works: SourceWork[];
  workTotal: number;
  cachedAt: number;
  warning: string;
  categories: string[];
  orthographies: string[];
}
