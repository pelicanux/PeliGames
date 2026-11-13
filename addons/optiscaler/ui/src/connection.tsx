import type {AddonHost} from "../../../_infra/ui-sdk";
import {waitForAddonJob} from "../../../_infra/ui-sdk/helpers/jobs";
import type {OptiScalerEnvironment} from "./environment";
import {messages} from "./messages";
import projects from "../projects.json";

/** Typed ports connect this interface to its selected external service. */
export function connectOptiScaler(host: AddonHost): OptiScalerEnvironment {
  const translate = host.i18n.createTranslator(messages);
  const call = <T,>(method: string, request: unknown = null) => host.service.call<T>(method, request);
  const mutate = <T,>(directory: string, action: unknown) => call<T>("game.mutate", {directory, action});
  return {
    t: (_namespace, key, values) => translate(key, values),
    ui: {
      ...host.ui,
      LaunchProfile: ({game, ...props}) => <host.ui.LaunchProfile {...props} gameId={game.path}/>,
      Projects: () => <host.ui.Projects projects={projects}/>,
    },
    openDirectory: host.files.openDirectory,
    synchronizeLaunch: async (game, directory, line) => host.game ? host.game.synchronizeLaunch(game.path, directory, line) : false,
    api: {
      inspect: directory => call("game.inspect", {directory}),
      install: (directory, proxy, settings) => mutate(directory, {operation: "install", proxy, settings}),
      upgrade: directory => mutate(directory, {operation: "upgrade"}),
      remove: directory => mutate(directory, {operation: "remove"}),
      update: (directory, key, value) => mutate(directory, {operation: "update", key, value}),
      setPatcher: async (directory, enabled) => {
        const operation = await mutate<{job: number}>(directory, {operation: "patcher", enabled});
        await waitForAddonJob(host, operation.job);
      },
      runtimeState: () => call("runtime.inspect"),
      setChannel: channel => call("runtime.channel", {channel}),
      removeDownload: channel => call("runtime.remove", {channel}),
      download: async (channel, progress) => {
        const operation = await call<{job: number}>("runtime.download", {channel});
        await waitForAddonJob(host, operation.job, progress);
      },
    },
  };
}
