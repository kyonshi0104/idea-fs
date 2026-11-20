import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Activity,
  Archive,
  ChevronDown,
  ChevronUp,
  Clock3,
  Folder,
  Inbox,
  Plus,
  X,
} from "lucide-react";
import { useLanguage, type TranslationKey } from "./i18n";
import "./App.css";

type Metadata = {
  title: string;
  tags: string[];
  status: string;
  created_at: string;
  icon?: string | null;
  resource_path?: string | null;
  app_type?: string | null;
  url?: string | null;
};

type IdeaFile = {
  filepath: string;
  metadata: Metadata;
};

type IdeaFolder = {
  name: string;
  status: string;
  files: IdeaFile[];
};

type TreeData = {
  folders: IdeaFolder[];
};

type FileData = IdeaFile & {
  content: string;
};

type SortField = "created_at" | "title";
type DocumentMode = "closed" | "viewer" | "editor";
type ContextMenu = { file: IdeaFile; x: number; y: number };
type CategoryContextMenu = { category: IdeaFolder; x: number; y: number };
type CategoryDialog = { mode: "create" } | { mode: "edit"; category: IdeaFolder };
type HideableDefaultStatus = "backlog" | "wip" | "graveyard";
type CategoryVisibility = Record<HideableDefaultStatus, boolean>;
type Theme = "default-dark" | "light" | "nord" | "mocha";
type AppSettings = {
  data_directory: string;
  supports_mac_cli_install: boolean;
  mac_cli_installed: boolean;
};

function shortcutFromKeyboardEvent(event: KeyboardEvent): string | null {
  const modifierParts: string[] = [];
  if (event.ctrlKey || event.metaKey) modifierParts.push("CommandOrControl");
  if (event.altKey) modifierParts.push("Alt");
  if (event.shiftKey) modifierParts.push("Shift");
  if (modifierParts.length === 0 || event.key.length === 0) return null;

  const key = event.key === " " ? "Space" : event.key.length === 1
    ? event.key.toUpperCase()
    : event.key;
  return [...modifierParts, key].join("+");
}

const THEMES: { value: Theme; labelKey: TranslationKey; colors: string[] }[] = [
  {
    value: "default-dark",
    labelKey: "theme.defaultDark",
    colors: ["#1e1f22", "#252629", "#3767a5", "#6ea7ff"],
  },
  {
    value: "light",
    labelKey: "theme.light",
    colors: ["#f4f6f9", "#e9edf2", "#2867ad", "#2765b0"],
  },
  {
    value: "nord",
    labelKey: "theme.nord",
    colors: ["#2e3440", "#3b4252", "#5e81ac", "#88c0d0"],
  },
  {
    value: "mocha",
    labelKey: "theme.mocha",
    colors: ["#1e1e2e", "#242438", "#7956a8", "#cba6f7"],
  },
];

function storedTheme(): Theme {
  const value =
    localStorage.getItem("ideafs-theme") ??
    localStorage.getItem("idea-explorer-theme");
  return THEMES.some((theme) => theme.value === value)
    ? (value as Theme)
    : "default-dark";
}

const STATUS_LABELS: Record<string, TranslationKey> = {
  inbox: "status.inbox",
  backlog: "status.backlog",
  wip: "status.wip",
  graveyard: "status.graveyard",
};

const DEFAULT_CATEGORY_LABELS: Record<string, TranslationKey> = {
  "01_Inbox": "status.inbox",
  "02_Backlog": "status.backlog",
  "03_WIP": "status.wip",
  "04_Graveyard": "status.graveyard",
};

const HIDEABLE_DEFAULT_STATUSES: HideableDefaultStatus[] = [
  "backlog",
  "wip",
  "graveyard",
];

function storedCategoryVisibility(): CategoryVisibility {
  return {
    backlog: localStorage.getItem("ideafs-show-backlog") !== "false",
    wip: localStorage.getItem("ideafs-show-wip") !== "false",
    graveyard: localStorage.getItem("ideafs-show-graveyard") !== "false",
  };
}

function isHideableDefaultStatus(status: string): status is HideableDefaultStatus {
  return status === "backlog" || status === "wip" || status === "graveyard";
}

function categoryLabel(category: IdeaFolder, t: (key: TranslationKey) => string) {
  const defaultLabel = DEFAULT_CATEGORY_LABELS[category.name];
  return defaultLabel ? t(defaultLabel) : category.name.replace(/^\d+_/, "");
}

function categoryIcon(status: string) {
  switch (status) {
    case "inbox":
      return Inbox;
    case "backlog":
      return Clock3;
    case "wip":
      return Activity;
    case "graveyard":
      return Archive;
    default:
      return Folder;
  }
}

function resourceActionLabel(
  appType: string | null | undefined,
  t: (key: TranslationKey) => string,
) {
  const knownLabels: Record<string, string> = {
    vscode: "VS Code",
    ableton: "Ableton Live",
    explorer: "File Explorer",
  };
  const type = appType?.trim();
  if (!type) return t("viewer.openResource");
  return t("viewer.openIn").replace("{app}", knownLabels[type.toLowerCase()] ?? type);
}

function isHttpImageUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

function isLocalImagePath(value: string) {
  const absolute = value.startsWith("/") || /^[a-z]:[\\/]/i.test(value) || /^\\\\/.test(value);
  return absolute && /\.(png|jpe?g|gif|webp|avif|svg)$/i.test(value);
}

function IdeaIcon({
  value,
  className,
  onError,
}: {
  value?: string | null;
  className: string;
  onError: (message: string) => void;
}) {
  const { t } = useLanguage();
  const [source, setSource] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    setSource(null);
    setFailed(false);
    if (value && isHttpImageUrl(value)) {
      setSource(value);
    } else if (value && isLocalImagePath(value)) {
      void invoke<string>("read_icon", { path: value })
        .then((dataUrl) => {
          if (active) setSource(dataUrl);
        })
        .catch((reason: unknown) => {
          if (active) {
            setFailed(true);
            onError(`${t("icon.loadFailed")}: ${String(reason)}`);
          }
        });
    }
    return () => {
      active = false;
    };
  }, [value, onError, t]);

  if (source && !failed) {
    return (
      <img
        className={className}
        src={source}
        alt=""
        onError={() => {
          setFailed(true);
          onError(t("icon.loadImageFailed"));
        }}
      />
    );
  }

  const image = Boolean(value && (isHttpImageUrl(value) || isLocalImagePath(value)));
  return <span className={className}>{value && !image ? value : "▤"}</span>;
}

function App() {
  const { language, setLanguage, t } = useLanguage();
  const [tree, setTree] = useState<IdeaFolder[]>([]);
  const [activeStatus, setActiveStatus] = useState("inbox");
  const [selected, setSelected] = useState<FileData | null>(null);
  const [contentLoaded, setContentLoaded] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [sortField, setSortField] = useState<SortField>("created_at");
  const [sortAscending, setSortAscending] = useState(false);
  const [propertiesOpen, setPropertiesOpen] = useState(true);
  const [documentMode, setDocumentMode] = useState<DocumentMode>("closed");
  const [contextMenu, setContextMenu] = useState<ContextMenu | null>(null);
  const [categoryMenu, setCategoryMenu] = useState<CategoryContextMenu | null>(null);
  const [categoryDialog, setCategoryDialog] = useState<CategoryDialog | null>(null);
  const [categoryName, setCategoryName] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [newTags, setNewTags] = useState("");
  const [newStatus, setNewStatus] = useState("inbox");
  const [newContent, setNewContent] = useState("");
  const [newIcon, setNewIcon] = useState("");
  const [newResourcePath, setNewResourcePath] = useState("");
  const [newAppType, setNewAppType] = useState("");
  const [newUrl, setNewUrl] = useState("");
  const [theme, setTheme] = useState<Theme>(storedTheme);
  const [categoryVisibility, setCategoryVisibility] =
    useState<CategoryVisibility>(storedCategoryVisibility);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<"general" | "appearance">("general");
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [globalShortcut, setGlobalShortcut] = useState<string | null>(null);
  const [shortcutRecording, setShortcutRecording] = useState(false);
  const [shortcutError, setShortcutError] = useState("");
  const [updateBusy, setUpdateBusy] = useState(false);
  const updateCheckStarted = useRef(false);
  const reportIconError = useCallback((message: string) => setError(message), []);

  const refreshTree = useCallback(async () => {
    const data = await invoke<TreeData>("get_idea_tree");
    setTree(data.folders);
    return data.folders;
  }, []);

  useEffect(() => {
    void refreshTree().catch((reason: unknown) => setError(String(reason)));
  }, [refreshTree]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void listen("ideas-changed", () => {
      void refreshTree().catch((reason: unknown) => setError(String(reason)));
    })
      .then((stopListening) => {
        if (active) unlisten = stopListening;
        else stopListening();
      })
      .catch((reason: unknown) => setError(String(reason)));
    return () => {
      active = false;
      unlisten?.();
    };
  }, [refreshTree]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("ideafs-theme", theme);
  }, [theme]);

  useEffect(() => {
    for (const status of HIDEABLE_DEFAULT_STATUSES) {
      localStorage.setItem(`ideafs-show-${status}`, String(categoryVisibility[status]));
    }
  }, [categoryVisibility]);

  const refreshSettings = useCallback(async () => {
    const settings = await invoke<AppSettings>("get_app_settings");
    setAppSettings(settings);
  }, []);

  useEffect(() => {
    void refreshSettings().catch((reason: unknown) => setError(String(reason)));
  }, [refreshSettings]);

  const refreshGlobalShortcut = useCallback(async () => {
    const shortcut = await invoke<string | null>("get_global_shortcut");
    setGlobalShortcut(shortcut);
  }, []);

  useEffect(() => {
    void refreshGlobalShortcut().catch((reason: unknown) => setError(String(reason)));
  }, [refreshGlobalShortcut]);

  const checkForUpdates = useCallback(async (manual: boolean) => {
    if (updateBusy) return;
    setUpdateBusy(true);
    try {
      const update = await check();
      if (!update) {
        if (manual) {
          await message(t("update.noUpdate"), { title: "IdeaFS", kind: "info" });
        }
        return;
      }

      const approved = await ask(
        t("update.confirm").replace("{version}", update.version),
        { title: "IdeaFS", kind: "info" },
      );
      if (!approved) return;
      await update.downloadAndInstall();
      await relaunch();
    } catch (reason) {
      setError(`${t("update.failed")}: ${String(reason)}`);
    } finally {
      setUpdateBusy(false);
    }
  }, [t, updateBusy]);

  useEffect(() => {
    if (import.meta.env.DEV || updateCheckStarted.current) return;
    updateCheckStarted.current = true;
    void checkForUpdates(false);
  }, [checkForUpdates]);

  const saveGlobalShortcut = async (shortcut: string | null) => {
    setShortcutError("");
    try {
      await invoke("set_global_shortcut", { shortcut });
      setGlobalShortcut(shortcut);
      setShortcutRecording(false);
    } catch (reason) {
      setShortcutError(String(reason));
    }
  };

  const captureGlobalShortcut = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    if (event.key === "Escape") {
      setShortcutRecording(false);
      setShortcutError("");
      return;
    }
    const shortcut = shortcutFromKeyboardEvent(event.nativeEvent);
    if (!shortcut) {
      setShortcutError(t("shortcut.invalid"));
      return;
    }
    void saveGlobalShortcut(shortcut);
  };

  const installMacCliCommand = async () => {
    setBusy(true);
    setError("");
    try {
      await invoke("install_mac_cli_command");
      await refreshSettings();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const activeFolder = tree.find((folder) => folder.status === activeStatus);
  const categoryStatusLabel = (status: string) => {
    const category = tree.find((folder) => folder.status === status);
    if (category) return categoryLabel(category, t);
    return STATUS_LABELS[status] ? t(STATUS_LABELS[status]) : status.replace(/^\d+_/, "");
  };
  const visibleCategories = tree.filter((category) => {
    return !isHideableDefaultStatus(category.status) ||
      categoryVisibility[category.status];
  });
  const visibleFiles = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    const files = (activeFolder?.files ?? []).filter((file) => {
      if (!needle) return true;
      return [file.metadata.title, ...file.metadata.tags]
        .join(" ")
        .toLocaleLowerCase()
        .includes(needle);
    });

    files.sort((left, right) => {
      const compared =
        sortField === "title"
          ? left.metadata.title.localeCompare(right.metadata.title)
          : Date.parse(left.metadata.created_at) - Date.parse(right.metadata.created_at);
      return (sortAscending ? 1 : -1) * compared;
    });
    return files;
  }, [activeFolder, search, sortAscending, sortField]);

  const openDocument = async (
    filepath: string,
    mode: Exclude<DocumentMode, "closed">,
  ) => {
    if (busy) return;
    if (
      dirty &&
      selected?.filepath !== filepath &&
      !window.confirm(t("confirm.discard"))
    ) {
      return;
    }
    const preserveMetadata = dirty && selected?.filepath === filepath;
    if (selected?.filepath === filepath && contentLoaded) {
      setDocumentMode(mode);
      return;
    }

    setBusy(true);
    setError("");
    try {
      const data = await invoke<FileData>("read_idea_file", { filepath });
      setSelected((current) =>
        preserveMetadata && current?.filepath === filepath
          ? { ...data, metadata: current.metadata }
          : data,
      );
      if (!preserveMetadata) {
        setActiveStatus(data.metadata.status);
        setDirty(false);
      }
      setContentLoaded(true);
      setDocumentMode(mode);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const openViewer = (filepath: string) => openDocument(filepath, "viewer");
  const openEditor = (filepath: string) => openDocument(filepath, "editor");

  const saveFile = async () => {
    if (!selected || !dirty || busy) return;
    setBusy(true);
    setError("");
    try {
      let filepath = selected.filepath;
      let content = selected.content;
      if (!contentLoaded) {
        const file = await invoke<FileData>("read_idea_file", { filepath });
        content = file.content;
      }

      const folder = tree.find((item) => item.status === selected.metadata.status);
      if (!folder) throw new Error(t("error.unknownStatus"));
      if (filepath.split("/")[0] !== folder.name) {
        filepath = await invoke<string>("move_idea_file", {
          sourcePath: filepath,
          targetDir: folder.name,
        });
      }
      await invoke("save_idea_file", {
        filepath,
        metadata: selected.metadata,
        content,
      });

      const [data] = await Promise.all([
        invoke<FileData>("read_idea_file", { filepath }),
        refreshTree(),
      ]);
      setSelected(data);
      setActiveStatus(data.metadata.status);
      setContentLoaded(true);
      setDirty(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const moveFile = async (sourcePath: string, targetDirectory: string) => {
    if (busy || sourcePath.split("/")[0] === targetDirectory) return;
    if (
      dirty &&
      selected?.filepath === sourcePath &&
      !window.confirm(t("confirm.discardMove"))
    ) {
      return;
    }

    setBusy(true);
    setError("");
    try {
      const filepath = await invoke<string>("move_idea_file", {
        sourcePath,
        targetDir: targetDirectory,
      });
      await refreshTree();
      if (selected?.filepath === sourcePath) {
        const data = await invoke<FileData>("read_idea_file", { filepath });
        setSelected(data);
        setActiveStatus(data.metadata.status);
        setContentLoaded(true);
        setDirty(false);
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const createIdea = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!newTitle.trim() || busy) return;
    setBusy(true);
    setError("");
    try {
      const filepath = await invoke<string>("create_idea", {
        title: newTitle.trim(),
        tags: newTags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        status: newStatus,
        content: newContent,
        icon: newIcon.trim() || null,
        resourcePath: newResourcePath.trim() || null,
        appType: newAppType.trim() || null,
        url: newUrl.trim() || null,
      });
      const [data] = await Promise.all([
        invoke<FileData>("read_idea_file", { filepath }),
        refreshTree(),
      ]);
      setSelected(data);
      setActiveStatus(data.metadata.status);
      setContentLoaded(true);
      setDirty(false);
      setDocumentMode("closed");
      setPropertiesOpen(true);
      setCreateOpen(false);
      setNewTitle("");
      setNewTags("");
      setNewContent("");
      setNewIcon("");
      setNewResourcePath("");
      setNewAppType("");
      setNewUrl("");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const submitCategory = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!categoryDialog || !categoryName.trim() || busy) return;
    setBusy(true);
    setError("");
    try {
      if (categoryDialog.mode === "create") {
        await invoke<string>("create_category", { name: categoryName.trim() });
      } else {
        const { category } = categoryDialog;
        const newDirectory = await invoke<string>("rename_category", {
          currentDir: category.name,
          name: categoryName.trim(),
        });
        const nextStatus = STATUS_LABELS[category.status]
          ? category.status
          : newDirectory;
        setActiveStatus((current) =>
          current === category.status ? nextStatus : current,
        );
        setSelected((current) => {
          const oldPrefix = `${category.name}/`;
          if (!current?.filepath.startsWith(oldPrefix)) return current;
          return {
            ...current,
            filepath: `${newDirectory}${current.filepath.slice(category.name.length)}`,
            metadata: { ...current.metadata, status: nextStatus },
          };
        });
      }
      await refreshTree();
      setCategoryName("");
      setCategoryDialog(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && createOpen) setCreateOpen(false);
      if (event.key === "Escape" && contextMenu) setContextMenu(null);
      if (event.key === "Escape" && categoryMenu) setCategoryMenu(null);
      if (event.key === "Escape" && categoryDialog) setCategoryDialog(null);
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void saveFile();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  const selectStatus = (status: string) => {
    if (status === activeStatus || busy) return;
    if (dirty && !window.confirm(t("confirm.discard"))) return;
    setActiveStatus(status);
    setSelected(null);
    setContentLoaded(false);
    setDirty(false);
    setDocumentMode("closed");
  };

  const setDefaultCategoryVisibility = (
    status: HideableDefaultStatus,
    visible: boolean,
  ) => {
    if (!visible && activeStatus === status) {
      if (dirty && !window.confirm(t("confirm.discard"))) return;
      setActiveStatus("inbox");
      setSelected(null);
      setContentLoaded(false);
      setDirty(false);
      setDocumentMode("closed");
    }
    setCategoryVisibility((current) => ({ ...current, [status]: visible }));
  };

  const selectFile = (file: IdeaFile) => {
    if (busy) return;
    if (dirty && selected?.filepath !== file.filepath) {
      if (!window.confirm(t("confirm.discard"))) return;
      setDirty(false);
    }
    setSelected((current) =>
      current?.filepath === file.filepath
        ? current
        : { ...file, content: "" },
    );
    setContentLoaded((current) => selected?.filepath === file.filepath && current);
  };

  const updateMetadata = (updates: Partial<Metadata>) => {
    setSelected((current) =>
      current ? { ...current, metadata: { ...current.metadata, ...updates } } : current,
    );
    setDirty(true);
  };

  const openResource = async () => {
    const path = selected?.metadata.resource_path?.trim();
    if (!path || busy) return;
    setBusy(true);
    setError("");
    try {
      await invoke("open_resource", { path });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const openUrl = async () => {
    const url = selected?.metadata.url?.trim();
    if (!url || busy) return;
    setBusy(true);
    setError("");
    try {
      await invoke("open_url", { url });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const deleteIdea = async (file: IdeaFile, skipConfirmation: boolean) => {
    if (busy) return;
    const hasUnsavedChanges = dirty && selected?.filepath === file.filepath;
    if (
      !skipConfirmation &&
      !window.confirm(
        `${t("confirm.delete").replace("{title}", file.metadata.title)}${
          hasUnsavedChanges ? `\n${t("confirm.deleteUnsaved")}` : ""
        }`,
      )
    ) {
      setContextMenu(null);
      return;
    }

    setBusy(true);
    setError("");
    setContextMenu(null);
    try {
      await invoke("delete_idea_file", { filepath: file.filepath });
      if (selected?.filepath === file.filepath) {
        setSelected(null);
        setContentLoaded(false);
        setDirty(false);
        setDocumentMode("closed");
      }
      await refreshTree();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const deleteCategory = async (category: IdeaFolder) => {
    if (busy || STATUS_LABELS[category.status]) return;
    setCategoryMenu(null);
    if (
      !window.confirm(
        t("confirm.deleteCategory").replace("{name}", categoryLabel(category, t)),
      )
    ) {
      return;
    }

    setBusy(true);
    setError("");
    try {
      await invoke("delete_category", { categoryDir: category.name });
      await refreshTree();
      if (activeStatus === category.status) {
        setActiveStatus("inbox");
        setSelected(null);
        setContentLoaded(false);
        setDirty(false);
        setDocumentMode("closed");
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const showContextMenu = (event: ReactMouseEvent<HTMLButtonElement>, file: IdeaFile) => {
    event.preventDefault();
    if (busy) return;
    setCategoryMenu(null);
    if (
      dirty &&
      selected?.filepath !== file.filepath &&
      !window.confirm(t("confirm.discard"))
    ) {
      return;
    }
    if (selected?.filepath !== file.filepath) {
      setSelected({ ...file, content: "" });
      setContentLoaded(false);
      setDirty(false);
      setDocumentMode("closed");
    }
    setContextMenu({
      file,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 180)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 100)),
    });
  };

  const showCategoryContextMenu = (
    event: ReactMouseEvent<HTMLButtonElement>,
    category: IdeaFolder,
  ) => {
    event.preventDefault();
    if (busy) return;
    setContextMenu(null);
    setCategoryMenu({
      category,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 180)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 100)),
    });
  };

  return (
    <div className="app-shell">
      <header className="topbar">
        <button
          className="primary-button"
          disabled={busy}
          onClick={() => {
            setNewStatus(activeStatus);
            setError("");
            setCreateOpen(true);
          }}
        >
          <span aria-hidden="true">＋</span> {t("app.newIdea")}
        </button>
        <button
          className="secondary-button category-add-topbar-button"
          disabled={busy}
          onClick={() => {
            setCategoryName("");
            setError("");
            setCategoryDialog({ mode: "create" });
          }}
        >
          <Plus size={15} aria-hidden="true" />
          {t("category.add")}
        </button>
        <span className="topbar-spacer" />
        {dirty && <span className="dirty-indicator">{t("app.unsavedChanges")}</span>}
        <button
          className="save-button"
          disabled={!selected || !dirty || busy}
          onClick={() => void saveFile()}
          title={`${t("common.save")} (Ctrl+S)`}
        >
          {t("common.save")}
        </button>
        <button
          className="settings-button"
          onClick={() => {
            setSettingsTab("general");
            setSettingsOpen(true);
          }}
          aria-label={t("app.settings")}
          title={t("app.settings")}
        >
          {t("app.settings")}
        </button>
      </header>

      <div className={`workspace ${propertiesOpen ? "with-properties" : ""}`}>
        <aside className="sidebar" aria-label={t("app.ideaStatuses")}>
          <nav className="status-list">
            {visibleCategories.map((category) => {
              const Icon = categoryIcon(category.status);
              return (
                <button
                  className={`status-item ${
                    activeStatus === category.status ? "active" : ""
                  }`}
                  key={category.name}
                  onClick={() => selectStatus(category.status)}
                  onContextMenu={(event) => showCategoryContextMenu(event, category)}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => {
                    event.preventDefault();
                    const sourcePath = event.dataTransfer.getData("text/idea-file");
                    if (sourcePath) void moveFile(sourcePath, category.name);
                  }}
                >
                  <Icon className="category-icon" size={16} aria-hidden="true" />
                  <span className="category-label">{categoryLabel(category, t)}</span>
                  <span className="status-count">{category.files.length}</span>
                </button>
              );
            })}
          </nav>
        </aside>

        <main className="main-pane">
          {error && (
            <div className="error-banner" role="alert">
              <span>{error}</span>
              <button onClick={() => setError("")} aria-label={t("common.dismissError")}>
                <X size={14} aria-hidden="true" />
              </button>
            </div>
          )}

          {documentMode === "editor" && selected ? (
            <section className="full-editor">
              <div className="list-toolbar editor-toolbar">
                <button className="text-button" onClick={() => setDocumentMode("closed")}>
                  {t("editor.backToProjects")}
                </button>
                <span className="file-path">{selected.filepath}</span>
              </div>
              <div className="editor-content">
                <input
                  className="title-input"
                  aria-label={t("editor.ideaTitle")}
                  value={selected.metadata.title}
                  onChange={(event) => updateMetadata({ title: event.target.value })}
                  placeholder={t("editor.ideaTitle")}
                />
                <div className="editor-metadata">
                  <label className="editor-field">
                    <span>{t("common.icon")}</span>
                    <input
                      value={selected.metadata.icon ?? ""}
                      onChange={(event) =>
                        updateMetadata({ icon: event.target.value.trim() || undefined })
                      }
                      placeholder={t("editor.iconPlaceholder")}
                    />
                  </label>
                  <label className="editor-field">
                    <span>{t("common.status")}</span>
                    <select
                      value={selected.metadata.status}
                      onChange={(event) => updateMetadata({ status: event.target.value })}
                    >
                      {tree.map((category) => (
                        <option value={category.status} key={category.name}>
                          {categoryLabel(category, t)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="editor-field editor-tags">
                    <span>{t("common.tags")}</span>
                    <input
                      value={selected.metadata.tags.join(", ")}
                      onChange={(event) =>
                        updateMetadata({
                          tags: event.target.value
                            .split(",")
                            .map((tag) => tag.trim())
                            .filter(Boolean),
                        })
                      }
                      placeholder={t("editor.tagsPlaceholder")}
                    />
                  </label>
                  <label className="editor-field editor-wide">
                    <span>{t("common.resource")}</span>
                    <input
                      value={selected.metadata.resource_path ?? ""}
                      onChange={(event) =>
                        updateMetadata({ resource_path: event.target.value.trim() || undefined })
                      }
                      placeholder={t("editor.resourcePlaceholder")}
                    />
                  </label>
                  <label className="editor-field">
                    <span>{t("common.appHint")}</span>
                    <input
                      value={selected.metadata.app_type ?? ""}
                      onChange={(event) =>
                        updateMetadata({ app_type: event.target.value.trim() || undefined })
                      }
                      placeholder={t("editor.appHintPlaceholder")}
                    />
                  </label>
                  <label className="editor-field editor-wide">
                    <span>{t("common.relatedLink")}</span>
                    <input
                      value={selected.metadata.url ?? ""}
                      onChange={(event) =>
                        updateMetadata({ url: event.target.value.trim() || undefined })
                      }
                      placeholder={t("editor.relatedUrlPlaceholder")}
                    />
                  </label>
                  <div className="editor-created">
                    {t("editor.createdAt").replace("{date}", selected.metadata.created_at)}
                  </div>
                </div>
                <label className="content-heading" htmlFor="markdown-editor">
                  {t("editor.markdown")}
                </label>
                <textarea
                  id="markdown-editor"
                  className="markdown-editor"
                  value={selected.content}
                  onChange={(event) => {
                    setSelected((current) =>
                      current ? { ...current, content: event.target.value } : current,
                    );
                    setDirty(true);
                  }}
                  placeholder={t("editor.notesPlaceholder")}
                  spellCheck={false}
                />
              </div>
            </section>
          ) : documentMode === "viewer" && selected ? (
            <section className="idea-viewer">
              <div className="list-toolbar viewer-toolbar">
                <button className="text-button" onClick={() => setDocumentMode("closed")}>
                  {t("editor.backToProjects")}
                </button>
                <span className="file-path">{selected.filepath}</span>
                <button
                  className="primary-button"
                  disabled={busy}
                  onClick={() => setDocumentMode("editor")}
                >
                  {t("common.edit")}
                </button>
              </div>
              <div className="viewer-scroll">
                <div className="viewer-document">
                  <div className="viewer-title-row">
                    <IdeaIcon
                      className="viewer-icon"
                      value={selected.metadata.icon}
                      onError={reportIconError}
                    />
                    <h1>{selected.metadata.title}</h1>
                  </div>
                  <div className="viewer-metadata">
                    <span>
                      {categoryStatusLabel(selected.metadata.status)}
                    </span>
                    <span>
                      {new Date(selected.metadata.created_at).toLocaleString(
                        language === "ja" ? "ja-JP" : "en-US",
                      )}
                    </span>
                    {selected.metadata.tags.map((tag) => (
                      <span className="tag-chip" key={tag}>{tag}</span>
                    ))}
                  </div>
                  {selected.metadata.resource_path && (
                    <div className="viewer-reference">
                      <span>{t("common.resource")}</span>
                      <span>{selected.metadata.resource_path}</span>
                    </div>
                  )}
                  {selected.metadata.url && (
                    <div className="viewer-reference">
                      <span>{t("common.link")}</span>
                      <a href={selected.metadata.url} target="_blank" rel="noreferrer">
                        {selected.metadata.url}
                      </a>
                    </div>
                  )}
                  <article className="markdown-body">
                    {selected.content.trim() ? (
                      <ReactMarkdown
                        remarkPlugins={[remarkGfm]}
                        components={{
                          a: ({ href, children }) => (
                            <a href={href} target="_blank" rel="noreferrer">
                              {children}
                            </a>
                          ),
                        }}
                      >
                        {selected.content}
                      </ReactMarkdown>
                    ) : (
                      <p className="viewer-empty">{t("viewer.noNotes")}</p>
                    )}
                  </article>
                </div>
              </div>
            </section>
          ) : (
            <>
              <div className="list-toolbar">
                <div className="section-title">
                  <h1>
                    {activeFolder
                      ? categoryLabel(activeFolder, t)
                      : activeStatus.replace(/^\d+_/, "")}
                  </h1>
                  <span>
                    {t("list.projectsCount").replace("{count}", String(visibleFiles.length))}
                  </span>
                </div>
                <div className="list-controls">
                  <label className="search-field">
                    <span aria-hidden="true">⌕</span>
                    <input
                      aria-label={t("list.filterProjects")}
                      value={search}
                      onChange={(event) => setSearch(event.target.value)}
                      placeholder={t("list.filterProjects")}
                    />
                    {search && (
                      <button
                        className="clear-search"
                        aria-label={t("list.clearFilter")}
                        onClick={() => setSearch("")}
                      >
                        <X size={14} aria-hidden="true" />
                      </button>
                    )}
                  </label>
                  <label className="sort-field">
                    <span>{t("list.sort")}</span>
                    <select
                      aria-label={t("list.sort")}
                      value={sortField}
                      onChange={(event) => setSortField(event.target.value as SortField)}
                    >
                      <option value="created_at">{t("sort.created")}</option>
                      <option value="title">{t("sort.title")}</option>
                    </select>
                  </label>
                  <button
                    className="sort-direction"
                    onClick={() => setSortAscending((current) => !current)}
                    aria-label={`${t("list.sort")} ${t(
                      sortAscending ? "list.ascending" : "list.descending",
                    )}`}
                    title={`${t("list.sort")} ${t(
                      sortAscending ? "list.ascending" : "list.descending",
                    )}`}
                  >
                    {sortAscending
                      ? <ChevronUp size={16} aria-hidden="true" />
                      : <ChevronDown size={16} aria-hidden="true" />}
                  </button>
                  <button
                    className={`properties-toggle ${propertiesOpen ? "active" : ""}`}
                    onClick={() => setPropertiesOpen((current) => !current)}
                    aria-expanded={propertiesOpen}
                  >
                    {t("list.properties")}
                  </button>
                </div>
              </div>

              <div className="project-list">
                <div className="list-header">
                  <span>{t("list.project")}</span>
                  <span>{t("common.tags")}</span>
                  <span>{t("common.created")}</span>
                </div>
                {visibleFiles.map((file) => (
                  <button
                    className={`project-row ${
                      selected?.filepath === file.filepath ? "selected" : ""
                    }`}
                    key={file.filepath}
                    disabled={busy}
                    draggable={!busy}
                    onDragStart={(event) =>
                      event.dataTransfer.setData("text/idea-file", file.filepath)
                    }
                    onClick={() => selectFile(file)}
                    onDoubleClick={() => void openViewer(file.filepath)}
                    onContextMenu={(event) => showContextMenu(event, file)}
                    title={t("list.rowHint")}
                  >
                    <span className="project-name">
                      <IdeaIcon
                        className="row-icon"
                        value={file.metadata.icon}
                        onError={reportIconError}
                      />
                      <span>{file.metadata.title}</span>
                    </span>
                    <span className="tag-list">
                      {file.metadata.tags.length ? (
                        file.metadata.tags.map((tag) => (
                          <span className="tag-chip" key={tag}>
                            {tag}
                          </span>
                        ))
                      ) : (
                        <span className="muted-value">—</span>
                      )}
                    </span>
                    <span className="created-value">
                      {new Date(file.metadata.created_at).toLocaleDateString(
                        language === "ja" ? "ja-JP" : "en-US",
                      )}
                    </span>
                  </button>
                ))}
                {visibleFiles.length === 0 && (
                  <div className="list-empty">
                    <span className="empty-icon">▤</span>
                    <strong>
                      {search ? t("list.noMatchingProjects") : t("list.noProjects")}
                    </strong>
                    <span>
                      {search
                        ? t("list.tryDifferentFilter")
                        : t("list.emptyHelp")}
                    </span>
                  </div>
                )}
              </div>
            </>
          )}
          {busy && <div className="busy-indicator">{t("common.working")}</div>}
        </main>

        {propertiesOpen && (
          <aside className="properties-pane" aria-label={t("list.properties")}>
            <div className="properties-heading">
              <span>{t("properties.heading")}</span>
              <button
                className="icon-button"
                onClick={() => setPropertiesOpen(false)}
                aria-label={t("properties.close")}
                title={t("properties.close")}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </div>
            {selected ? (
              <div className="properties-content">
                <div className="property-hero">
                  <IdeaIcon
                    className="property-icon"
                    value={selected.metadata.icon}
                    onError={reportIconError}
                  />
                  <h2>{selected.metadata.title || t("properties.untitled")}</h2>
                </div>
                <div className="property-section">
                  <div className="property-section-title">{t("properties.details")}</div>
                  <dl className="property-list">
                    <div className="property-row">
                      <dt>{t("common.status")}</dt>
                      <dd>
                        <span className={`property-status status-${selected.metadata.status}`} />
                        {categoryStatusLabel(selected.metadata.status)}
                      </dd>
                    </div>
                    <div className="property-row">
                      <dt>{t("common.tags")}</dt>
                      <dd className="property-tags">
                        {selected.metadata.tags.length ? (
                          selected.metadata.tags.map((tag) => (
                            <span className="tag-chip" key={tag}>{tag}</span>
                          ))
                        ) : (
                          <span className="property-muted">—</span>
                        )}
                      </dd>
                    </div>
                    <div className="property-row">
                      <dt>{t("common.created")}</dt>
                      <dd>
                        {new Date(selected.metadata.created_at).toLocaleString(
                          language === "ja" ? "ja-JP" : "en-US",
                        )}
                      </dd>
                    </div>
                    {selected.metadata.resource_path && (
                      <div className="property-row">
                        <dt>{t("common.resource")}</dt>
                        <dd className="property-break">{selected.metadata.resource_path}</dd>
                      </div>
                    )}
                    {selected.metadata.app_type && (
                      <div className="property-row">
                        <dt>{t("common.appHint")}</dt>
                        <dd>{selected.metadata.app_type}</dd>
                      </div>
                    )}
                    {selected.metadata.url && (
                      <div className="property-row">
                        <dt>{t("common.relatedLink")}</dt>
                        <dd className="property-break">{selected.metadata.url}</dd>
                      </div>
                    )}
                    <div className="property-row">
                      <dt>{t("common.file")}</dt>
                      <dd className="property-break">{selected.filepath}</dd>
                    </div>
                  </dl>
                </div>
                {(selected.metadata.resource_path?.trim() || selected.metadata.url?.trim()) && (
                  <div className="property-actions">
                    {selected.metadata.resource_path?.trim() && (
                      <button
                        className="open-action"
                        disabled={busy}
                        onClick={() => void openResource()}
                      >
                        {resourceActionLabel(selected.metadata.app_type, t)}
                      </button>
                    )}
                    {selected.metadata.url?.trim() && (
                      <button
                        className="open-action"
                        disabled={busy}
                        onClick={() => void openUrl()}
                      >
                        {t("viewer.openLink")}
                      </button>
                    )}
                  </div>
                )}
              </div>
            ) : (
              <div className="property-empty">{t("properties.selectProject")}</div>
            )}
          </aside>
        )}
      </div>

      {contextMenu && (
        <div className="context-menu-backdrop" onMouseDown={() => setContextMenu(null)}>
          <div
            className="context-menu"
            role="menu"
            style={{ left: contextMenu.x, top: contextMenu.y }}
            onMouseDown={(event) => event.stopPropagation()}
          >
            <button
              role="menuitem"
              disabled={busy}
              onClick={() => {
                const filepath = contextMenu.file.filepath;
                setContextMenu(null);
                void openEditor(filepath);
              }}
            >
              {t("context.edit")}
            </button>
            <div className="context-menu-separator" />
            <button
              className="danger"
              role="menuitem"
              onClick={(event) => void deleteIdea(contextMenu.file, event.shiftKey)}
            >
              {t("context.delete")}
              <span>{navigator.platform.includes("Mac") ? "⇧" : "Shift"}</span>
            </button>
          </div>
        </div>
      )}

      {categoryMenu && (
        <div
          className="context-menu-backdrop"
          onMouseDown={() => setCategoryMenu(null)}
        >
          <div
            className="context-menu"
            role="menu"
            style={{ left: categoryMenu.x, top: categoryMenu.y }}
            onMouseDown={(event) => event.stopPropagation()}
          >
            <button
              role="menuitem"
              disabled={busy}
              onClick={() => {
                const category = categoryMenu.category;
                setCategoryMenu(null);
                setCategoryName(category.name.replace(/^\d+_/, ""));
                setError("");
                setCategoryDialog({ mode: "edit", category });
              }}
            >
              {t("category.edit")}
            </button>
            <div className="context-menu-separator" />
            <button
              className="danger"
              role="menuitem"
              disabled={busy || Boolean(STATUS_LABELS[categoryMenu.category.status])}
              title={
                STATUS_LABELS[categoryMenu.category.status]
                  ? t("category.protected")
                  : undefined
              }
              onClick={() => void deleteCategory(categoryMenu.category)}
            >
              {t("category.delete")}
            </button>
          </div>
        </div>
      )}

      {categoryDialog && (
        <div
          className="modal-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget && !busy) {
              setCategoryDialog(null);
            }
          }}
        >
          <section
            className="create-modal category-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="category-dialog-title"
          >
            <div className="modal-heading">
              <h2 id="category-dialog-title">
                {categoryDialog.mode === "create"
                  ? t("category.addTitle")
                  : t("category.editTitle")}
              </h2>
              <button
                className="icon-button"
                onClick={() => setCategoryDialog(null)}
                disabled={busy}
                aria-label={t("common.close")}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </div>
            {error && (
              <div className="error-banner modal-error" role="alert">
                <span>{error}</span>
                <button onClick={() => setError("")} aria-label={t("common.dismissError")}>
                  <X size={14} aria-hidden="true" />
                </button>
              </div>
            )}
            <form onSubmit={(event) => void submitCategory(event)}>
              <input
                autoFocus
                aria-label={t("category.namePlaceholder")}
                value={categoryName}
                onChange={(event) => setCategoryName(event.target.value)}
                placeholder={t("category.namePlaceholder")}
                maxLength={64}
                required
              />
              <div className="modal-actions">
                <button
                  className="secondary-button"
                  type="button"
                  disabled={busy}
                  onClick={() => setCategoryDialog(null)}
                >
                  {t("common.cancel")}
                </button>
                <button
                  className="primary-button"
                  type="submit"
                  disabled={busy || !categoryName.trim()}
                >
                  {categoryDialog.mode === "create"
                    ? t("category.create")
                    : t("common.save")}
                </button>
              </div>
            </form>
          </section>
        </div>
      )}

      {createOpen && (
        <div
          className="modal-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget && !busy) setCreateOpen(false);
          }}
        >
          <section
            className="create-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="create-title"
          >
            <div className="modal-heading">
              <div>
                <h2 id="create-title">{t("create.newIdea")}</h2>
              </div>
              <button
                className="icon-button"
                onClick={() => setCreateOpen(false)}
                disabled={busy}
                aria-label={t("common.close")}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </div>
            {error && (
              <div className="error-banner modal-error" role="alert">
                <span>{error}</span>
                <button onClick={() => setError("")} aria-label={t("common.dismissError")}>
                  <X size={14} aria-hidden="true" />
                </button>
              </div>
            )}
            <form onSubmit={(event) => void createIdea(event)}>
              <label className="modal-field">
                <span>{t("common.title")} <em>{t("create.required")}</em></span>
                <input
                  autoFocus
                  value={newTitle}
                  onChange={(event) => setNewTitle(event.target.value)}
                  placeholder={t("create.titlePlaceholder")}
                  required
                />
              </label>
              <label className="modal-field">
                <span>{t("common.status")}</span>
                <select
                  value={newStatus}
                  onChange={(event) => setNewStatus(event.target.value)}
                >
                  {tree.map((category) => (
                    <option value={category.status} key={category.name}>
                      {categoryLabel(category, t)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="modal-field">
                <span>{t("common.tags")}</span>
                <input
                  value={newTags}
                  onChange={(event) => setNewTags(event.target.value)}
                  placeholder={t("create.tagsPlaceholder")}
                />
              </label>
              <div className="modal-field-grid">
                <label className="modal-field">
                  <span>{t("common.icon")}</span>
                  <input
                    value={newIcon}
                    onChange={(event) => setNewIcon(event.target.value)}
                    placeholder={t("editor.iconPlaceholder")}
                  />
                </label>
                <label className="modal-field">
                  <span>{t("common.appHint")}</span>
                  <input
                    value={newAppType}
                    onChange={(event) => setNewAppType(event.target.value)}
                    placeholder={t("editor.appHintPlaceholder")}
                  />
                </label>
              </div>
              <label className="modal-field">
                <span>{t("common.resource")}</span>
                <input
                  value={newResourcePath}
                  onChange={(event) => setNewResourcePath(event.target.value)}
                  placeholder={t("editor.resourcePlaceholder")}
                />
              </label>
              <label className="modal-field">
                <span>{t("common.relatedLink")}</span>
                <input
                  value={newUrl}
                  onChange={(event) => setNewUrl(event.target.value)}
                  placeholder={t("create.relatedUrlPlaceholder")}
                />
              </label>
              <label className="modal-field">
                <span>{t("common.notes")}</span>
                <textarea
                  value={newContent}
                  onChange={(event) => setNewContent(event.target.value)}
                  placeholder={t("create.notesPlaceholder")}
                  rows={4}
                />
              </label>
              <div className="modal-actions">
                <button
                  className="secondary-button"
                  type="button"
                  disabled={busy}
                  onClick={() => setCreateOpen(false)}
                >
                  {t("common.cancel")}
                </button>
                <button className="primary-button" type="submit" disabled={busy}>
                  {t("create.submit")}
                </button>
              </div>
            </form>
          </section>
        </div>
      )}

      {settingsOpen && (
        <div
          className="modal-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget && !busy) setSettingsOpen(false);
          }}
        >
          <section
            className="create-modal settings-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="settings-title"
          >
            <div className="modal-heading">
              <div>
                <span className="eyebrow">{t("settings.preferences")}</span>
                <h2 id="settings-title">{t("settings.title")}</h2>
              </div>
              <button
                className="icon-button"
                onClick={() => setSettingsOpen(false)}
                disabled={busy}
                aria-label={t("common.close")}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </div>
            <div className="settings-tabs" role="tablist">
              <button
                className={settingsTab === "general" ? "active" : ""}
                role="tab"
                aria-selected={settingsTab === "general"}
                onClick={() => setSettingsTab("general")}
              >
                {t("settings.general")}
              </button>
              <button
                className={settingsTab === "appearance" ? "active" : ""}
                role="tab"
                aria-selected={settingsTab === "appearance"}
                onClick={() => setSettingsTab("appearance")}
              >
                {t("settings.appearance")}
              </button>
            </div>
            {settingsTab === "general" ? (
              <div className="settings-panel">
                <label className="modal-field">
                  <span>{t("settings.dataDirectory")}</span>
                  <input
                    readOnly
                    value={appSettings?.data_directory ?? t("settings.loading")}
                  />
                </label>
                <label className="modal-field">
                  <span>{t("settings.language")}</span>
                  <select
                    value={language}
                    onChange={(event) => {
                      const value = event.target.value;
                      if (value === "en" || value === "ja") setLanguage(value);
                    }}
                  >
                    <option value="en">{t("settings.english")}</option>
                    <option value="ja">{t("settings.japanese")}</option>
                  </select>
                </label>
                <div className="settings-category-visibility">
                  <span className="settings-label">{t("settings.visibleCategories")}</span>
                  {HIDEABLE_DEFAULT_STATUSES.map((status) => (
                    <label className="settings-category-toggle" key={status}>
                      <span>{t(STATUS_LABELS[status])}</span>
                      <input
                        type="checkbox"
                        disabled={busy}
                        checked={categoryVisibility[status]}
                        onChange={(event) =>
                          setDefaultCategoryVisibility(status, event.target.checked)
                        }
                      />
                    </label>
                  ))}
                </div>
                <div className="settings-shortcut">
                  <span className="settings-label">{t("shortcut.title")}</span>
                  <div className="settings-shortcut-controls">
                    <input
                      readOnly
                      value={
                        shortcutRecording
                          ? t("shortcut.pressKeys")
                          : globalShortcut ?? t("shortcut.disabled")
                      }
                      onClick={() => {
                        setShortcutError("");
                        setShortcutRecording(true);
                      }}
                      onKeyDown={captureGlobalShortcut}
                      aria-label={t("shortcut.title")}
                    />
                    <button
                      className="secondary-button"
                      disabled={busy || globalShortcut === null}
                      onClick={() => void saveGlobalShortcut(null)}
                    >
                      {t("shortcut.disable")}
                    </button>
                  </div>
                  <span className="settings-hint">{t("shortcut.help")}</span>
                  {shortcutError && (
                    <span className="settings-inline-error" role="alert">
                      {shortcutError}
                    </span>
                  )}
                </div>
                {appSettings?.supports_mac_cli_install && (
                  <div className="settings-cli">
                    <div>
                      <strong>{t("settings.commandLineAccess")}</strong>
                      <span>
                        {appSettings.mac_cli_installed
                          ? t("settings.cliInstalled")
                          : t("settings.cliInstallHelp")}
                      </span>
                    </div>
                    <button
                      className="secondary-button"
                      disabled={busy || appSettings.mac_cli_installed}
                      onClick={() => void installMacCliCommand()}
                    >
                      {appSettings.mac_cli_installed
                        ? t("settings.installed")
                        : t("settings.installCli")}
                    </button>
                  </div>
                )}
                <div className="settings-cli settings-updater">
                  <div>
                    <strong>{t("update.title")}</strong>
                    <span>{t("update.help")}</span>
                  </div>
                  <button
                    className="secondary-button"
                    disabled={busy || updateBusy}
                    onClick={() => void checkForUpdates(true)}
                  >
                    {updateBusy ? t("common.working") : t("update.check")}
                  </button>
                </div>
              </div>
            ) : (
              <div className="settings-panel">
                <span className="settings-label">{t("settings.colorTheme")}</span>
                <div className="theme-options">
                  {THEMES.map(({ value, labelKey, colors }) => (
                    <button
                      className={`theme-option ${theme === value ? "selected" : ""}`}
                      key={value}
                      aria-pressed={theme === value}
                      aria-label={t(labelKey)}
                      onClick={() => setTheme(value)}
                    >
                      <span className="theme-option-label">{t(labelKey)}</span>
                      <span className="theme-palette" aria-hidden="true">
                        {colors.map((color, index) => (
                          <span
                            className="theme-swatch"
                            key={index}
                            style={{ backgroundColor: color }}
                          />
                        ))}
                      </span>
                      {theme === value && (
                        <span className="theme-check" aria-hidden="true">✓</span>
                      )}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </section>
        </div>
      )}
    </div>
  );
}

export default App;
