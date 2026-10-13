#!/usr/bin/env bash
# Guiado e interativo. Nunca recebe o Client Secret como argumento ou variável.
set -euo pipefail
set +x

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd -- "$script_dir"
if [[ ! -t 0 ]]; then
  printf '%s\n' 'Execute este script em um terminal interativo: bash tools/discord-auth/setup.sh' >&2
  exit 1
fi
if command -v bunx >/dev/null 2>&1; then
  wrangler=(bunx wrangler)
elif [[ -x "${HOME}/.bun/bin/bunx" ]]; then
  wrangler=("${HOME}/.bun/bin/bunx" wrangler)
elif command -v npx >/dev/null 2>&1; then
  wrangler=(npx wrangler)
else
  printf '%s\n' 'Não encontrei Bun/bunx nem Node.js/npx. Instale uma dessas ferramentas e execute novamente.' >&2
  exit 1
fi
printf '\n%s\n' 'Configuração do login Discord — PeliGames' \
  'Este script cadastra o segredo e publica o Worker peligames-discord-auth na sua conta Cloudflare.' \
  'O serviço de relatórios não será alterado.' \
  'Tenha aberto o Discord Developer Portal → PeliGames → OAuth2.'
printf '\n%s\n' '1/4 — Verificando sua conta Cloudflare…'
# whoami pode concluir sem erro mesmo quando não existe sessão; o deploy/secret
# também oferece login. Evitamos interpretar ou salvar a saída da conta.
"${wrangler[@]}" whoami
printf '\n%s\n' 'Se não apareceu a conta Cloudflare correta, faça login agora.'
read -r -p 'Abrir login do Cloudflare? [s/N]: ' do_login
case "$do_login" in
  s|S|sim|SIM) "${wrangler[@]}" login ;;
esac
printf '\n%s\n' '2/4 — Cadastro do Client Secret' \
  'No Discord Developer Portal → OAuth2, copie o Client Secret da aplicação PeliGames.' \
  'Se não houver um segredo disponível, use Reset Secret para gerar um novo.' \
  'Cole SOMENTE quando o Wrangler pedir abaixo. A digitação fica oculta.' \
  'Não cole a chave em comandos, arquivos ou neste chat.'
"${wrangler[@]}" secret put DISCORD_CLIENT_SECRET --config "$script_dir/wrangler.toml"
printf '\n%s\n' '3/4 — Publicando o serviço de login…'
"${wrangler[@]}" deploy --config "$script_dir/wrangler.toml"
printf '\n%s\n' '4/4 — URL de retorno' \
  'Copie a origem HTTPS mostrada no deploy (sem /callback e sem barra final).'
read -r -p 'URL do Worker: ' relay_origin
relay_origin="${relay_origin%/}"
if [[ ! "$relay_origin" =~ ^https://[a-zA-Z0-9]([a-zA-Z0-9.-]*[a-zA-Z0-9])?$ ]]; then
  printf '%s\n' 'A URL deve ser somente a origem HTTPS, por exemplo https://peligames-discord-auth.sua-conta.workers.dev' >&2
  exit 1
fi
printf '\n%s\n' 'No Discord Developer Portal → OAuth2 → Redirects → Add Redirect, cadastre:'
printf '%s/callback\n' "$relay_origin"
printf '%s\n' 'Clique em Save Changes.'
printf '\n%s\n' 'Envie apenas esta URL pública no chat para ligarmos o launcher ao serviço:'
printf '%s\n' "$relay_origin"
printf '\n%s\n' 'O Client Secret está no Cloudflare; este script não o salva localmente.' \
  'Ainda é necessário configurar a URL pública no build do launcher e testar o login real.'
