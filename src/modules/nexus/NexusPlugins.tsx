import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
interface Plugin {
  name: string; enabled: boolean; native: boolean; index: string | null; managed: string | null;
  master: boolean; light: boolean; medium: boolean; archive: boolean;
  masters: string[]; missing: string[]; messages: { kind: "say" | "warn" | "error"; text: string }[];
  dirty: string[]; incompatible: string[]; requires: string[]; group: string | null;
  rules: { kind: "after" | "requires" | "incompatible"; plugin: string }[]; ghost: boolean; light_choice: boolean | null;
}
interface View { groups: string[]; plugins: Plugin[]; autosort: boolean; loot: boolean; light: boolean; medium: boolean; counts: [number, number, number]; limits: [number, number, number]; list: string }
/** Plugins page of the Bethesda games, matching the Vortex plugin management page. */
export function NexusPlugins({ game, disabled, onClose }: { game: NexusGame; disabled: boolean; onClose: () => void }) {
  const text = useNexusText();
  const [view, setView] = useState<View | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [ruleKind, setRuleKind] = useState<"after" | "requires" | "incompatible">("after");
  const [ruleTarget, setRuleTarget] = useState("");
  const [newGroup, setNewGroup] = useState("");
  const [newGroupAfter, setNewGroupAfter] = useState("default");
  useEffect(() => { setRuleTarget(""); }, [open]);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    void invoke<View>("nexus_plugins", { gameId: game.id }).then(value => { if (active.current) setView(value); }).catch(e => { if (active.current) setError(String(e)); });
    return () => { active.current = false; };
  }, [game.id]);
  const run = (command: string, args: Record<string, unknown> = {}) => {
    if (working) return;
    setWorking(true); setError("");
    void invoke<View>(command, { gameId: game.id, ...args }).then(value => { if (active.current) setView(value); })
      .catch(e => { if (active.current) setError(String(e)); }).finally(() => { if (active.current) setWorking(false); });
  };
  const locked = disabled || working || !view;
  const move = (name: string, up: boolean) => {
    if (!view) return;
    const order = view.plugins.map(plugin => plugin.name), index = order.indexOf(name), target = index + (up ? -1 : 1);
    if (target < 0 || target >= order.length || view.plugins[target].native) return;
    [order[index], order[target]] = [order[target], order[index]];
    run("set_nexus_plugin_order", { order });
  };
  const over = (index: number) => view && view.counts[index] > view.limits[index];
  const shown = (view?.plugins || []).filter(plugin => plugin.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  return <NexusPopup className="nexus-load-order-popup nexus-plugins-popup" title={text.plugins} onClose={onClose}>
    <p className="nexus-hint">{text.pluginsHint}</p>
    {view && <div className="nexus-tabs">
      <label className="nexus-fomod-option"><input type="checkbox" checked={view.autosort} disabled={locked} onChange={event => run("set_nexus_plugin_autosort", { enabled: event.target.checked })} />{text.pluginsAutosort}</label>
      <HoverTooltip text={view.loot ? text.pluginsAutosortHint : text.pluginsNoMasterlist}><button type="button" className="installation-type-info" aria-label={text.pluginsAutosortHint}><MenuIcon name="info" /></button></HoverTooltip>
      <button type="button" className="btn btn-primary" disabled={locked || !view.loot} onClick={() => run("sort_nexus_plugins")}><MenuIcon name="sliders" />{working ? text.loading : text.pluginsSortNow}</button>
      <button type="button" className="btn btn-secondary" onClick={() => void invoke("open_folder", { path: view.list }).catch(e => setError(String(e)))}><MenuIcon name="logs" />{text.loadOrderOpenFile}</button>
    </div>}
    {view && <p className={`nexus-hint ${over(0) || over(1) || over(2) ? "nexus-error" : ""}`} role="status">
      {text.pluginsActive}: {view.counts[0]}/{view.limits[0]}{view.light && ` · Light: ${view.counts[1]}/${view.limits[1]}`}{view.medium && ` · Medium: ${view.counts[2]}/${view.limits[2]}`}
      {(over(0) || over(1) || over(2)) && ` · ${text.pluginsLimit}`}
    </p>}
    {view && view.plugins.length > 8 && <label className="nexus-mod-search-field"><MenuIcon name="search" /><input type="search" aria-label={text.pluginsSearch} placeholder={text.pluginsSearch} value={query} onChange={event => setQuery(event.target.value)} /></label>}
    {view && !view.plugins.length && <p className="nexus-hint">{text.pluginsEmpty}</p>}
    {view && shown.length > 0 && <ol className="nexus-load-order-list">{shown.map(plugin => {
      const position = view.plugins.indexOf(plugin);
      const warnings = plugin.missing.length + plugin.incompatible.length + plugin.requires.length + plugin.messages.filter(m => m.kind !== "say").length + plugin.dirty.length;
      const flags = [plugin.ghost && "Ghost", plugin.light_choice !== null && text.pluginLightChosen, plugin.native && text.pluginNative, plugin.master && "Master", plugin.light && "Light", plugin.medium && "Medium", plugin.archive && "BSA", plugin.group && `${text.pluginGroup}: ${plugin.group}`].filter(Boolean).join(" · ");
      return <li key={plugin.name} className={`nexus-mod-row nexus-load-order-row ${plugin.enabled ? "" : "locked"}`}>
        <input type="checkbox" aria-label={`${plugin.enabled ? text.disable : text.enable}: ${plugin.name}`} checked={plugin.enabled} disabled={locked || plugin.native} onChange={event => run("set_nexus_plugin_enabled", { name: plugin.name, enabled: event.target.checked })} />
        <span className="nexus-load-order-index">{plugin.index ?? "—"}</span>
        <span className="nexus-installed-mod-info">
          <button type="button" className="nexus-plugin-name" aria-expanded={open === plugin.name} onClick={() => setOpen(current => current === plugin.name ? null : plugin.name)}><strong>{plugin.name}</strong>{warnings > 0 && <span className="nexus-error"> ⚠ {warnings}</span>}</button>
          <small>{plugin.managed ?? text.loadOrderExternal}{flags && ` · ${flags}`}</small>
          {plugin.missing.length > 0 && <small className="nexus-error">{text.pluginMissingMasters}: {plugin.missing.join(", ")}</small>}
          {plugin.incompatible.length > 0 && <small className="nexus-error">{text.pluginIncompatible}: {plugin.incompatible.join(", ")}</small>}
          {plugin.requires.length > 0 && <small className="nexus-error">{text.pluginRequires}: {plugin.requires.join(", ")}</small>}
          {open === plugin.name && <>
            {plugin.masters.length > 0 && <small>{text.pluginMasters}: {plugin.masters.join(", ")}</small>}
            {plugin.dirty.map(item => <small key={item} className="nexus-error">{text.pluginDirty}: {item}</small>)}
            {plugin.messages.map((message, index) => <small key={index} className={message.kind === "say" ? "" : "nexus-error"}>LOOT · {message.text}</small>)}
            <div className="nexus-plugin-details">
              <strong>{text.pluginRules}</strong>
              {plugin.rules.map(rule => <span key={`${rule.kind}:${rule.plugin}`} className="nexus-plugin-rule">{text[`pluginRule_${rule.kind}`]} {rule.plugin}
                <button type="button" className="btn btn-secondary" aria-label={`${text.remove}: ${rule.plugin}`} disabled={locked} onClick={() => run("set_nexus_plugin_rule", { plugin: plugin.name, kind: rule.kind, target: rule.plugin, add: false })}><MenuIcon name="trash" /></button></span>)}
              <span className="nexus-plugin-rule">
                <select aria-label={text.pluginRules} value={ruleKind} disabled={locked} onChange={event => setRuleKind(event.target.value as typeof ruleKind)}>{(["after", "requires", "incompatible"] as const).map(kind => <option key={kind} value={kind}>{text[`pluginRule_${kind}`]}</option>)}</select>
                <select aria-label={text.pluginRuleTarget} value={ruleTarget} disabled={locked} onChange={event => setRuleTarget(event.target.value)}><option value="">{text.pluginRuleTarget}</option>{view.plugins.filter(other => other.name !== plugin.name).map(other => <option key={other.name} value={other.name}>{other.name}</option>)}</select>
                <button type="button" className="btn btn-secondary" disabled={locked || !ruleTarget} onClick={() => run("set_nexus_plugin_rule", { plugin: plugin.name, kind: ruleKind, target: ruleTarget, add: true })}><MenuIcon name="plusCircle" />{text.pluginRuleAdd}</button>
              </span>
              <strong>{text.pluginGroup}</strong>
              <span className="nexus-plugin-rule">
                <select aria-label={text.pluginGroup} value={plugin.group ?? ""} disabled={locked} onChange={event => run("set_nexus_plugin_group", { plugin: plugin.name, group: event.target.value || null })}><option value="">{text.pluginGroupDefault}</option>{view.groups.map(group => <option key={group} value={group}>{group}</option>)}</select>
                <input aria-label={text.pluginGroupNew} placeholder={text.pluginGroupNew} value={newGroup} disabled={locked} onChange={event => setNewGroup(event.target.value)} />
                <select aria-label={text.pluginGroupAfter} value={newGroupAfter} disabled={locked} onChange={event => setNewGroupAfter(event.target.value)}>{view.groups.map(group => <option key={group} value={group}>{text.pluginGroupAfter} {group}</option>)}</select>
                <button type="button" className="btn btn-secondary" disabled={locked || !newGroup.trim()} onClick={() => { run("add_nexus_plugin_group", { name: newGroup, after: newGroupAfter ? [newGroupAfter] : [] }); setNewGroup(""); }}><MenuIcon name="plusCircle" />{text.pluginGroupCreate}</button>
              </span>
              {!plugin.native && <span className="nexus-plugin-rule">
                <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => run("set_nexus_plugin_ghost", { name: plugin.name, ghost: !plugin.ghost })}><MenuIcon name="eye" />{plugin.ghost ? text.pluginUnghost : text.pluginGhost}</button>
                {view.light && plugin.managed && <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => run("set_nexus_plugin_light", { name: plugin.name, light: plugin.light_choice === null ? !plugin.light : null })}><MenuIcon name="bolt" />{plugin.light_choice !== null ? text.pluginLightReset : plugin.light ? text.pluginMarkRegular : text.pluginMarkLight}</button>}
              </span>}
            </div>
          </>}
        </span>
        {!view.autosort && !plugin.native && <span className="nexus-mod-order-actions">
          <button type="button" className="btn btn-secondary" aria-label={`${text.moveUp}: ${plugin.name}`} title={text.moveUp} disabled={locked || position === 0 || view.plugins[position - 1]?.native} onClick={() => move(plugin.name, true)}><MenuIcon name="chevronUp" /></button>
          <button type="button" className="btn btn-secondary" aria-label={`${text.moveDown}: ${plugin.name}`} title={text.moveDown} disabled={locked || position === view.plugins.length - 1} onClick={() => move(plugin.name, false)}><MenuIcon name="chevronDown" /></button>
        </span>}
      </li>;
    })}</ol>}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </NexusPopup>;
}
