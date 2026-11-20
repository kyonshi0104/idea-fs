import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { useLanguage } from "./i18n";
import "./QuickAdd.css";

export default function QuickAdd() {
  const { t } = useLanguage();
  const [title, setTitle] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    document.documentElement.dataset.theme =
      localStorage.getItem("ideafs-theme") ?? "default-dark";
    const close = () => {
      void getCurrentWindow().close();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("blur", close);
    window.addEventListener("keydown", onKeyDown);
    inputRef.current?.focus();
    return () => {
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const trimmedTitle = title.trim();
    if (!trimmedTitle || busy) return;
    setBusy(true);
    setError("");
    try {
      await invoke("create_idea", {
        title: trimmedTitle,
        tags: [],
        status: "inbox",
        content: "",
        icon: null,
        resourcePath: null,
        appType: null,
        url: null,
      });
      await emit("ideas-changed");
      await getCurrentWindow().close();
    } catch (reason) {
      setError(String(reason));
      setBusy(false);
      inputRef.current?.focus();
    }
  };

  return (
    <form className="quick-add" onSubmit={(event) => void submit(event)}>
      <input
        ref={inputRef}
        autoFocus
        value={title}
        onChange={(event) => setTitle(event.target.value)}
        placeholder={t("quickAdd.placeholder")}
        aria-label={t("quickAdd.placeholder")}
        aria-invalid={Boolean(error)}
        title={error || undefined}
        disabled={busy}
      />
    </form>
  );
}
