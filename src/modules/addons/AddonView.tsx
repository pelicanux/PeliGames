import {Component, useEffect, useMemo, useRef, useState, type ReactNode} from "react";
import type {AddonContext} from "../../../addons/_infra/ui-sdk";
import {useI18n} from "../../i18n/I18nContext";
import {addonBridge} from "./bridge";
import type {AddonRoute} from "./contracts";
import {importAddonModule} from "./moduleImport";
import {AddonSession, type OpenedAddon} from "./session";
import {ServiceLeases} from "./serviceLeases";
import {addonDesign} from "./sharedDesign";
import {useAddonGameActions} from "./gameHost";
import "./addonView.css";

const leases = new ServiceLeases(addonBridge);
class ViewBoundary extends Component<{children: ReactNode; fallback: ReactNode; onFailure: () => void}, {failed: boolean}> {
  state = {failed: false};
  static getDerivedStateFromError() {return {failed: true}}
  componentDidCatch() {this.props.onFailure()}
  render() {return this.state.failed ? this.props.fallback : this.props.children}
}
export function AddonView({route, game, disabled = false, onBusyChange}: {route: AddonRoute; game?: AddonContext["game"]; disabled?: boolean; onBusyChange?: (busy: boolean) => void}) {
  const {t, language} = useI18n();
  const gameActions = useAddonGameActions(route.id, game);
  const [opened, setOpened] = useState<{key: string; addon: OpenedAddon}>();
  const [status, setStatus] = useState<"loading" | "ready" | "failed" | "changed">("loading");
  const [retry, setRetry] = useState(0);
  const active = useRef<AddonSession | undefined>(undefined);
  const busy = useRef(onBusyChange); busy.current = onBusyChange;
  const requestKey = JSON.stringify([route.id, route.target, route.route, language, game?.id, game?.directory, game?.executableDirectory, game?.native, game?.running, retry]);
  const context = useMemo<AddonContext>(() => Object.freeze({target: route.target, route: route.route, ...(game ? {game: Object.freeze({...game})} : {})}), [route.target, route.route, game?.id, game?.directory, game?.executableDirectory, game?.native, game?.running]);
  useEffect(() => {
    setOpened(undefined); setStatus("loading");
    let mounted = true;
    const session = new AddonSession(addonBridge, leases, importAddonModule, addonDesign,
      reason => {if (mounted) {setOpened(undefined); setStatus(reason)}}, value => busy.current?.(value), 5000, gameActions);
    active.current = session;
    void session.open(route, language).then(value => {if (mounted && !session.scope.signal.aborted) {setOpened({key: requestKey, addon: value}); setStatus("ready")}}, () => {if (mounted) {setOpened(undefined); setStatus(current => current === "changed" ? current : "failed")}});
    return () => {mounted = false; void session.close(); if (active.current === session) active.current = undefined};
  }, [requestKey]);
  const failure = <div className="addon-view-message" role="alert"><p>{t("addons", status === "changed" ? "changed" : "failed")}</p><button className="mod-action-quiet" type="button" onClick={() => setRetry(value => value + 1)}>{t("addons", "retry")}</button></div>;
  if (status === "loading" || opened && opened.key !== requestKey) return <div className="addon-view-message" role="status">{t("addons", "loading")}</div>;
  if (!opened || status !== "ready") return failure;
  const {View} = opened.addon;
  return <section className="addon-view mod-workspace"><ViewBoundary key={requestKey} fallback={failure} onFailure={() => {void active.current?.close()}}><View context={context} disabled={disabled}/></ViewBoundary></section>;
}
