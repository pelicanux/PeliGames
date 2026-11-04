import { ConfirmModal } from "../../components/ConfirmModal";
import { LoadingModal } from "../../components/LoadingModal";
import { PeliRunProfile } from "../../components/PeliRunProfile";
import { ResultModal } from "../../components/ResultModal";
import { useI18n } from "../../i18n/I18nContext";
import type { useDlssInstallation } from "./useDlssInstallation";

interface Props {
  installation: ReturnType<typeof useDlssInstallation>;
  onUninstall: (path: string) => void;
  onSetup: () => void;
}

export function DlssInstallationDialogs({ installation, onUninstall, onSetup }: Props) {
  const { t } = useI18n();
  const { result, uninstallPath } = installation;
  return <>
    {uninstallPath && <ConfirmModal title={t("confirm", "attention")} message={t("confirm", "uninstallGame")}
      isDanger confirmText={t("confirm", "uninstallBtn")} cancelText={t("confirm", "cancel")}
      onCancel={() => installation.requestUninstall(null)}
      onConfirm={() => { installation.requestUninstall(null); onUninstall(uninstallPath); }} />}
    {result && <ResultModal title={result.title} message={result.message} type={result.type} logs={installation.logs}
      onClose={installation.closeResult}
      onOpenSetup={result.type === "error" && result.modelRecovery ? () => { installation.closeResult(); onSetup(); } : undefined}>
      {result.type === "success" && result.launchGame && <PeliRunProfile game={result.launchGame} setupOnly inline />}
    </ResultModal>}
    {installation.busy && installation.message && <LoadingModal message={installation.message} />}
  </>;
}
