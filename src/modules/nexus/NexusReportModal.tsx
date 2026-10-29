import { ReportModal } from "../reports/ReportModal";
export function NexusReportModal({gameId,onClose}: {gameId: string; onClose: () => void}) {
  return <ReportModal target={{kind:"nexus",gameId}} onClose={onClose} />;
}
