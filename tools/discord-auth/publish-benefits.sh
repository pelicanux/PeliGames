#!/usr/bin/env bash
set -euo pipefail
set +x
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd -- "$script_dir"
if command -v bunx >/dev/null 2>&1; then
  wrangler=(bunx wrangler)
elif [[ -x "${HOME}/.bun/bin/bunx" ]]; then
  wrangler=("${HOME}/.bun/bin/bunx" wrangler)
elif command -v npx >/dev/null 2>&1; then
  wrangler=(npx wrangler)
else
  printf '%s\n' 'Instale Bun ou Node.js para executar.' >&2; exit 1
fi
if command -v bun >/dev/null 2>&1; then
  runtime=(bun)
elif [[ -x "${HOME}/.bun/bin/bun" ]]; then
  runtime=("${HOME}/.bun/bin/bun")
else
  runtime=(node)
fi
printf '%s\n' 'Preparando assinatura das autorizações de sete dias no Cloudflare.'
"${runtime[@]}" "$script_dir/configure-signing.mjs" "${wrangler[@]}"
"${wrangler[@]}" deploy --config "$script_dir/wrangler.toml"
printf '%s\n' 'Publicado. Recompile o launcher para incluir a chave pública de verificação.'
