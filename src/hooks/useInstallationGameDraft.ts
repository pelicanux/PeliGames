import { useEffect, useState } from "react";
import { fetchGameCoverByName } from "../services/customCovers";

export function useInstallationGameDraft(automaticName = false) {
  const [name, setName] = useState("");
  const [draftName, setDraftName] = useState("");
  const [editing, setEditing] = useState(true);
  const [coverUrl, setCoverUrl] = useState<string>();
  const [coverStatus, setCoverStatus] = useState<"idle" | "loading" | "found" | "missing" | "error">("idle");
  const [revision, setRevision] = useState(0);
  const coverName = (automaticName || editing ? draftName : name).trim();

  useEffect(() => {
    if (!coverName) {
      if (automaticName) { setName(""); setEditing(true); }
      setCoverUrl(undefined);
      setCoverStatus("idle");
      return;
    }
    let active = true;
    setCoverUrl(undefined);
    setCoverStatus("loading");
    // Search after typing settles, without requiring confirmation of the draft.
    const timer = window.setTimeout(() => {
      if (automaticName) { setName(coverName); setEditing(false); }
      fetchGameCoverByName(coverName).then(url => {
        if (!active) return;
        setCoverUrl(url ?? undefined);
        setCoverStatus(url ? "found" : "missing");
      }).catch(() => {
        if (active) setCoverStatus("error");
      });
    }, 650);
    // A slower response must not replace artwork for a newer name.
    return () => { active = false; window.clearTimeout(timer); };
  }, [coverName, revision, automaticName]);

  return {
    name, draftName, editing, coverUrl, coverStatus,
    setDraftName: (value: string) => { setDraftName(value); if (automaticName) setEditing(value.trim() !== name); },
    reset: () => {
      setName(""); setDraftName(""); setEditing(true);
      setCoverUrl(undefined); setCoverStatus("idle"); setRevision(value => value + 1);
    },
    confirm: () => {
      const value = draftName.trim();
      if (!value) return;
      setName(value);
      setDraftName(value);
      setEditing(false);
      setRevision(value => value + 1);
    },
    beginEdit: () => { setDraftName(name); setEditing(true); },
    cancelEdit: () => { setDraftName(name); setEditing(false); },
    coverFailed: (url: string) => {
      if (url === coverUrl) { setCoverUrl(undefined); setCoverStatus("error"); }
    },
  };
}

export type InstallationGameDraft = ReturnType<typeof useInstallationGameDraft>;
