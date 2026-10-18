import type { BenchmarkStats } from "./mangoHudBenchmarks";
import { comparisonRows, comparisonValue } from "./mangoHudComparisonReport";
import { comparisonInsight, testLabel } from "./benchmarkPresentation";

/** Compact reading of the same recordings, without the detailed charts and descriptions. */
export function BenchmarkSimpleComparison({ results, labels }: { results: [BenchmarkStats | null, BenchmarkStats | null]; labels: [string, string] }) {
  return <div className="mm-simple-comparison">
    <table aria-label="Comparação simples dos benchmarks">
      <thead><tr><th scope="col">Métrica</th><th scope="col" className="mm-test-a">A · {testLabel(labels[0], 0)}</th><th scope="col" className="mm-test-b">B · {testLabel(labels[1], 1)}</th><th scope="col">Diferença em B</th></tr></thead>
      <tbody>{comparisonRows.map(row => {
        const insight = comparisonInsight(...results, row);
        return <tr key={row.key}>
          <th scope="row">{row.label}</th>
          <td className="mm-test-a">{comparisonValue(results[0]?.[row.key], row.unit)}</td>
          <td className="mm-test-b">{comparisonValue(results[1]?.[row.key], row.unit)}</td>
          <td data-tone={insight.tone}>{insight.text}</td>
        </tr>;
      })}</tbody>
    </table>
  </div>;
}
