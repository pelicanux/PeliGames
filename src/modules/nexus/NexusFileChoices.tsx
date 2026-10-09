import { groupNexusFiles, isObsoleteNexusFile } from "./fileGroups";
import { plainNexusText } from "./nexusLinks";
import { useNexusText } from "./text";
import type { QueueDetails } from "./queueGraph";
export function NexusFileChoices({ files, name, modName, selected, disabled, onSelect }: {
  files: QueueDetails["files"]; name: string; modName: string; selected: number | null; disabled: boolean; onSelect: (id: number) => void;
}) {
  const text = useNexusText();
  // An old file is shown only when explicitly chosen in the mod's file catalog.
  const available = files.filter(file => !isObsoleteNexusFile(file) || file.file_id === selected);
  const groups = groupNexusFiles(available);
  const chosen = available.find(file => file.file_id === selected);
  const render = (file: QueueDetails["files"][number]) => {
    const size = file.size < 1024 ? `${file.size.toFixed(1)} KB` : `${(file.size / 1024).toFixed(1)} MB`;
    const ambiguous = available.some(other => other.file_id !== file.file_id && other.name === file.name && other.version === file.version);
    const distinctName = file.name.trim().toLocaleLowerCase() !== modName.trim().toLocaleLowerCase();
    return <label className={`nexus-queue-file ${file.file_id === selected ? "selected" : ""}`} key={file.file_id} title={file.file_name}>
      <input type="radio" name={name} aria-label={`${file.name} · ${file.version} · ${size}`} checked={selected === file.file_id} disabled={disabled} onChange={() => onSelect(file.file_id)} />
      <span className="nexus-queue-file-info">{ambiguous ? <strong>{file.file_name}</strong> : distinctName && <strong>{file.name}</strong>}<small>{file.version} · {size}</small></span>
    </label>;
  };
  return <div className="nexus-queue-files" role="group" aria-label={text.queueChooseFile}>{groups.map(group => <section key={group.category} className="nexus-file-choice-group">
    {(groups.length > 1 || group.category === 4) && <h4>{group.category === 1 ? text.mainFiles : group.category === 3 ? text.optionalFiles : group.category === 2 ? text.patchFiles : group.category === 4 ? text.oldFiles : text.otherFiles}</h4>}
    {group.files.map(render)}
  </section>)}{chosen?.description && <div className="nexus-queue-selected-description">{plainNexusText(chosen.description)}</div>}{!available.length && <p className="nexus-hint">{text.noCatalogFiles}</p>}</div>;
}
