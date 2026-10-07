import { useEffect, useState } from "react";
import { fetchGameCoverByName } from "../services/customCovers";

export function useInstallationGameDraft() {
  const [name, setName] = useState("");
  const [draftName, setDraftName] = useState("");
  const [editing, setEditing] = useState(true);
  const [coverUrl, setCoverUrl] = useState<string>();
  const [coverStatus, setCoverStatus] = useState<"idle" | "loading" | "found" | "missing" | "error">("idle");
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    if (!name) return;
    let active = true;
    setCoverUrl(undefined);
    setCoverStatus("loading");
    fetchGameCoverByName(name).then(url => {
      if (!active) return;
      setCoverUrl(url ?? undefined);
      setCoverStatus(url ? "found" : "missing");
    }).catch(() => {
      if (active) setCoverStatus("error");
    });
    // Older searches cannot replace the cover after another name is confirmed.
    return () => { active = false; };
  }, [name, revision]);

  return {
    name, draftName, editing, coverUrl, coverStatus, setDraftName,
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
