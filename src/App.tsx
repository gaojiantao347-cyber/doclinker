import {
  isValidElement,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import Prism from "prismjs";
import "prismjs/components/prism-bash";
import "prismjs/components/prism-css";
import "prismjs/components/prism-java";
import "prismjs/components/prism-javascript";
import "prismjs/components/prism-json";
import "prismjs/components/prism-markdown";
import "prismjs/components/prism-rust";
import "prismjs/components/prism-sql";
import "prismjs/components/prism-typescript";
import "prismjs/components/prism-yaml";
import "./App.css";

const appWindow = getCurrentWebviewWindow();
const SEARCH_LIMIT = 20;
const SEARCH_DEBOUNCE_MS = 180;

type FileKind = "markdown" | "text" | "executable" | "url";

type SearchResult = {
  path: string;
  fileName: string;
  alias?: string | null;
  title?: string | null;
  fileKind: FileKind;
  workspaceKind: string;
  targetUrl?: string | null;
  score: number;
};

type FilePreview = {
  path: string;
  fileName: string;
  alias?: string | null;
  title?: string | null;
  fileKind: FileKind;
  workspaceKind: string;
  targetUrl?: string | null;
  content?: string | null;
};

type CodeProps = {
  children?: ReactNode;
  className?: string;
};

type PreProps = {
  children?: ReactNode;
};

const FIRST_CODE_BLOCK_PATTERN = /```[^\n]*\n([\s\S]*?)```/;

function App() {
  const [keyword, setKeyword] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isSearching, setIsSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const searchRequestRef = useRef(0);
  const previewRequestRef = useRef(0);

  const selectedResult = results[selectedIndex] ?? null;
  const activePreview = preview;
  const hasKeyword = keyword.trim().length > 0;
  const hasResultsPanel = hasKeyword;
  const hasPreviewPanel = Boolean(activePreview) || isPreviewLoading;

  useEffect(() => {
    const trimmed = keyword.trim();
    const requestId = searchRequestRef.current + 1;
    searchRequestRef.current = requestId;

    if (!trimmed) {
      setResults([]);
      setSelectedIndex(0);
      setPreview(null);
      setError(null);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    setError(null);

    const timer = window.setTimeout(() => {
      void invoke<SearchResult[]>("search", { keyword: trimmed, limit: SEARCH_LIMIT })
        .then((items) => {
          if (searchRequestRef.current !== requestId) {
            return;
          }
          setResults(items);
          setSelectedIndex(0);
          setPreview(null);
        })
        .catch((reason: unknown) => {
          if (searchRequestRef.current !== requestId) {
            return;
          }
          setResults([]);
          setSelectedIndex(0);
          setPreview(null);
          setError(toMessage(reason));
        })
        .finally(() => {
          if (searchRequestRef.current === requestId) {
            setIsSearching(false);
          }
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [keyword]);

  const showPreview = useCallback(async (result: SearchResult) => {
    if (!isPreviewable(result.fileKind)) {
      return;
    }

    const requestId = previewRequestRef.current + 1;
    previewRequestRef.current = requestId;
    setIsPreviewLoading(true);
    setError(null);
    try {
      const content = await invoke<FilePreview>("read_preview", { path: result.path });
      if (previewRequestRef.current === requestId) {
        setPreview(content);
      }
    } catch (reason) {
      if (previewRequestRef.current === requestId) {
        setError(toMessage(reason));
      }
    } finally {
      if (previewRequestRef.current === requestId) {
        setIsPreviewLoading(false);
      }
    }
  }, []);

  const copyToClipboard = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setNotice("已复制到剪贴板");
      window.setTimeout(() => setNotice(null), 1600);
      setError(null);
    } catch (reason) {
      setError(`复制失败：${toMessage(reason)}`);
    }
  }, []);

  const openResult = useCallback(async (result: SearchResult) => {
    if (result.fileKind === "url") {
      if (!result.targetUrl) {
        setError("URL 文件缺少目标地址");
        return;
      }
      await openUrl(result.targetUrl);
      return;
    }

    await openPath(result.path);
  }, []);

  const activateResult = useCallback(async (result: SearchResult | null) => {
    if (!result) {
      return;
    }

    try {
      if (isPreviewable(result.fileKind)) {
        if (preview?.path === result.path) {
          setPreview(null);
          return;
        }
        await showPreview(result);
        return;
      }
      await openResult(result);
    } catch (reason) {
      setError(toMessage(reason));
    }
  }, [openResult, preview?.path, showPreview]);

  const copyFirstCodeBlock = useCallback(async (result: SearchResult | null) => {
    if (!result || result.fileKind !== "markdown") {
      return false;
    }

    try {
      const markdown = await invoke<FilePreview>("read_preview", { path: result.path });
      const firstCodeBlock = extractFirstCodeBlock(markdown.content || "");
      if (!firstCodeBlock) {
        setError("当前 Markdown 未找到代码块");
        return true;
      }
      await copyToClipboard(firstCodeBlock);
      return true;
    } catch (reason) {
      setError(`提取代码块失败：${toMessage(reason)}`);
      return true;
    }
  }, [copyToClipboard]);

  useEffect(() => {
    const focusInput = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
    };

    focusInput();

    const unlistenPromise = appWindow.onFocusChanged(({ payload }) => {
      if (payload) {
        window.setTimeout(focusInput, 30);
      }
    });

    const onKeyDown = async (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key.toLowerCase() === "c") {
        if (document.activeElement !== inputRef.current && !isResultListFocused()) {
          return;
        }
        const handled = await copyFirstCodeBlock(selectedResult);
        if (handled) {
          event.preventDefault();
        }
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        await appWindow.hide();
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelectedIndex((index) => {
          if (results.length === 0) {
            return 0;
          }
          return (index + 1) % results.length;
        });
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((index) => {
          if (results.length === 0) {
            return 0;
          }
          return (index - 1 + results.length) % results.length;
        });
        return;
      }

      if (event.key === "Enter") {
        event.preventDefault();
        await activateResult(selectedResult);
      }
    };

    window.addEventListener("keydown", onKeyDown);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [activateResult, copyFirstCodeBlock, results.length, selectedResult]);

  useEffect(() => {
    if (selectedIndex >= results.length) {
      setSelectedIndex(Math.max(results.length - 1, 0));
    }
  }, [results.length, selectedIndex]);

  const previewTitle = useMemo(() => {
    if (activePreview) {
      return activePreview.title || activePreview.alias || activePreview.fileName;
    }
    if (selectedResult && isPreviewable(selectedResult.fileKind)) {
      return selectedResult.title || selectedResult.alias || selectedResult.fileName;
    }
    return "预览";
  }, [activePreview, selectedResult]);

  return (
    <main className={`min-h-screen bg-transparent p-5 text-zinc-100 ${hasResultsPanel ? "" : "grid place-items-center"}`}>
      <section className={`relative mx-auto grid w-full gap-3 ${hasPreviewPanel ? "h-[560px] max-w-[1040px] grid-cols-[420px_minmax(0,1fr)] grid-rows-[auto_minmax(0,1fr)]" : "max-w-[560px] grid-cols-1"}`}>
        <div className="rounded-[26px] border border-white/25 bg-zinc-950/84 px-6 py-5 shadow-[0_18px_60px_rgba(0,0,0,0.36)] backdrop-blur-xl">
          <div className="flex items-center gap-4">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full border border-white/15 bg-white/12 text-sm font-semibold tracking-[0.2em] text-white/85 shadow-[inset_0_1px_0_rgba(255,255,255,0.18)]">
              DL
            </div>
            <div className="min-w-0 flex-1">
              <label htmlFor="doclinker-search" className="sr-only">
                搜索文档、命令或脚本
              </label>
              <input
                ref={inputRef}
                id="doclinker-search"
                value={keyword}
                onChange={(event) => setKeyword(event.currentTarget.value)}
                placeholder="搜索文档、命令或脚本..."
                className="w-full border-0 bg-transparent text-[24px] font-medium tracking-[0.01em] text-white outline-none placeholder:text-white/55"
                autoComplete="off"
                spellCheck={false}
              />
              <p className="mt-1 text-xs tracking-[0.18em] text-white/60">
                ALT + F 打开 · ↑↓ 选择 · ENTER 预览/执行 · ESC 隐藏
              </p>
            </div>
          </div>
        </div>

        {error ? (
          <p className="pointer-events-none absolute left-3 top-[104px] z-30 w-[396px] rounded-2xl border border-red-400/25 bg-red-500/20 px-5 py-3 text-sm text-red-100 shadow-[0_18px_50px_rgba(0,0,0,0.42)] backdrop-blur-xl">
            {error}
          </p>
        ) : null}

        {notice ? (
          <p className="pointer-events-none absolute right-3 top-[104px] z-30 rounded-2xl border border-emerald-300/25 bg-emerald-400/18 px-5 py-3 text-sm text-emerald-50 shadow-[0_18px_50px_rgba(0,0,0,0.42)] backdrop-blur-xl">
            {notice}
          </p>
        ) : null}

        {hasResultsPanel ? (
          <div className={`${hasPreviewPanel ? "h-full" : "max-h-[560px]"} overflow-hidden rounded-[24px] border border-white/22 bg-zinc-950/82 shadow-[0_18px_60px_rgba(0,0,0,0.32)] backdrop-blur-xl`}>
            <ResultList
              results={results}
              selectedIndex={selectedIndex}
              isSearching={isSearching}
              onSelect={setSelectedIndex}
              onActivate={activateResult}
            />
          </div>
        ) : null}

        {hasPreviewPanel ? (
          <aside className="col-start-2 row-span-2 row-start-1 h-full overflow-hidden rounded-[24px] border border-white/22 bg-zinc-950/82 shadow-[0_18px_60px_rgba(0,0,0,0.32)] backdrop-blur-xl">
            <div className="flex h-[64px] items-center justify-between border-b border-white/10 px-6">
              <div className="min-w-0">
                <p className="text-xs uppercase tracking-[0.22em] text-white/55">
                  {activePreview?.fileKind === "text" ? "Text Preview" : "Markdown Preview"}
                </p>
                <h2 className="truncate text-base font-semibold text-white">{previewTitle}</h2>
              </div>
              {isPreviewLoading ? (
                <span className="rounded-full border border-white/16 bg-white/12 px-3 py-1 text-xs text-white/70">
                  加载中
                </span>
              ) : null}
            </div>
            <MarkdownPreview
              preview={activePreview}
              isLoading={isPreviewLoading}
              onCopyCode={copyToClipboard}
            />
          </aside>
        ) : null}
      </section>
    </main>
  );
}

function ResultList({
  results,
  selectedIndex,
  isSearching,
  onSelect,
  onActivate,
}: {
  results: SearchResult[];
  selectedIndex: number;
  isSearching: boolean;
  onSelect: (index: number) => void;
  onActivate: (result: SearchResult) => Promise<void>;
}) {
  if (isSearching && results.length === 0) {
    return <div className="px-6 py-5 text-sm text-white/45">正在搜索...</div>;
  }

  if (results.length === 0) {
    return <div className="px-6 py-5 text-sm text-white/45">没有匹配结果</div>;
  }

  return (
    <div className="glass-scrollbar h-full overflow-y-auto p-3" data-result-list>
      {results.map((result, index) => (
        <button
          key={result.path}
          type="button"
          onMouseEnter={() => onSelect(index)}
          onClick={() => onSelect(index)}
          onDoubleClick={() => void onActivate(result)}
          className={`mb-2 flex w-full items-start gap-3 rounded-2xl border px-3 py-3 text-left shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] transition ${
            index === selectedIndex
              ? "border-sky-200/55 bg-sky-300/22"
              : "border-white/14 bg-white/[0.08] hover:bg-white/[0.13]"
          }`}
        >
          <span className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-white/18 bg-white/12 text-xs font-bold text-white/85">
            {fileKindIcon(result.fileKind)}
          </span>
          <span className="min-w-0 flex-1">
            <span className="flex items-center gap-2">
              <span className="truncate text-sm font-semibold text-white">
                {result.title || result.alias || result.fileName}
              </span>
              <span className="rounded-full bg-white/12 px-2 py-0.5 text-[10px] uppercase tracking-[0.16em] text-white/60">
                {result.workspaceKind}
              </span>
            </span>
            {result.alias ? (
              <span className="mt-1 block truncate text-xs text-sky-100/75">别名：{result.alias}</span>
            ) : null}
            <span className="mt-1 block truncate text-xs text-white/60">{result.fileName}</span>
            <span className="mt-1 block truncate text-[11px] text-white/42">{result.path}</span>
          </span>
        </button>
      ))}
    </div>
  );
}

function MarkdownPreview({
  preview,
  isLoading,
  onCopyCode,
}: {
  preview: FilePreview | null;
  isLoading: boolean;
  onCopyCode: (code: string) => Promise<void>;
}) {
  if (!preview || isLoading) {
    return (
      <div className="flex h-[496px] items-center justify-center px-8 text-center text-sm leading-6 text-white/60">
        正在准备预览。
      </div>
    );
  }

  if (preview.fileKind === "text") {
    return (
      <div className="text-preview glass-scrollbar h-[496px] overflow-auto px-6 py-5">
        <pre>{preview.content || ""}</pre>
      </div>
    );
  }

  return (
    <div className="markdown-preview glass-scrollbar h-[496px] overflow-y-auto px-6 py-5">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          code: ({ children, className }) => (
            <CodeBlock className={className}>{children}</CodeBlock>
          ),
          pre: ({ children }) => (
            <PreBlock onCopy={onCopyCode}>{children}</PreBlock>
          ),
        }}
      >
        {preview.content || ""}
      </Markdown>
    </div>
  );
}

function PreBlock({ children, onCopy }: PreProps & {
  onCopy: (code: string) => Promise<void>;
}) {
  const codeElement = firstElement(children);
  const props = codeElement?.props as CodeProps | undefined;
  const className = props?.className;
  const match = /language-(\w+)/.exec(className || "");
  const language = match?.[1]?.toLowerCase() || "text";
  const source = toPlainText(props?.children ?? children).replace(/\n$/, "");

  return (
    <div className="code-action-block">
      <div className="code-action-block__bar">
        <span className="code-action-block__language">{language}</span>
        <div className="code-action-block__actions">
          <button type="button" onClick={() => void onCopy(source)}>复制</button>
        </div>
      </div>
      <pre>{children}</pre>
    </div>
  );
}

function CodeBlock({ children, className }: CodeProps) {
  const match = /language-(\w+)/.exec(className || "");
  const language = match?.[1]?.toLowerCase();
  const source = toPlainText(children).replace(/\n$/, "");
  const grammar = language ? Prism.languages[language] : null;

  if (!language || !grammar) {
    return <code className={className}>{children}</code>;
  }

  return (
    <code
      className={`language-${language}`}
      dangerouslySetInnerHTML={{ __html: Prism.highlight(source, grammar, language) }}
    />
  );
}

function isResultListFocused(): boolean {
  const activeElement = document.activeElement;
  return activeElement instanceof HTMLElement && activeElement.closest("[data-result-list]") !== null;
}

function extractFirstCodeBlock(markdown: string): string | null {
  return FIRST_CODE_BLOCK_PATTERN.exec(markdown)?.[1]?.replace(/\n$/, "") || null;
}

function firstElement(node: ReactNode): ReactElement | null {
  if (isValidElement(node)) {
    return node;
  }
  if (Array.isArray(node)) {
    for (const child of node) {
      const element = firstElement(child);
      if (element) {
        return element;
      }
    }
  }
  return null;
}

function toPlainText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === "boolean") {
    return "";
  }
  if (typeof node === "string" || typeof node === "number") {
    return String(node);
  }
  if (Array.isArray(node)) {
    return node.map(toPlainText).join("");
  }
  if (isValidElement(node)) {
    const element = node as ReactElement<{ children?: ReactNode }>;
    return toPlainText(element.props.children);
  }
  return "";
}

function isPreviewable(fileKind: FileKind): boolean {
  return fileKind === "markdown" || fileKind === "text";
}

function fileKindIcon(fileKind: FileKind): string {
  switch (fileKind) {
    case "markdown":
      return "MD";
    case "text":
      return "TXT";
    case "executable":
      return "EXE";
    case "url":
      return "URL";
  }
}

function toMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

export default App;
