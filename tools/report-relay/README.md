# report-relay

Ponte entre o botão **Enviar para o fórum** do PeliGames e o fórum de relatórios do Discord. O programa envia o relatório para cá, e a ponte o publica pelo webhook: cada jogo tem a sua postagem no fórum (criada no primeiro relatório), e os relatórios seguintes do jogo entram nela como mensagens novas. Se a postagem for apagada ou trancada, a ponte cria outra. A URL do webhook fica guardada só como segredo do Worker: não aparece no código, no repositório nem no PeliGames.

## O que a ponte confere

- O jogo vem da linha `Jogo:` do relatório.
- Só aceita `POST /report` com um relatório gerado pelo PeliGames (começa com `PELIGAMES — RELATÓRIO`), de até 9,5 MB.
- Até 3 relatórios por minuto por IP (`[[ratelimits]]` em `wrangler.toml`).
- Título, descrição e nome no Discord são limitados e limpos de caracteres de controle.
- Menções (`@everyone`, cargos, usuários) não notificam ninguém (`allowed_mentions` vazio).

## Publicação (uma vez)

1. **Webhook no Discord:** no canal de fórum, abra *Editar canal → Integrações → Webhooks → Novo webhook* e copie a URL. Não a cole em nenhum outro lugar.
2. **Ferramenta da Cloudflare:** crie uma conta gratuita em <https://dash.cloudflare.com> e ative a verificação em dois fatores. Nesta pasta, rode (o `bunx` vem com o Bun; com Node.js, use `npx`):
   ```
   bunx wrangler login
   ```
3. **Índice de postagens:** cada jogo tem uma postagem no fórum, e os relatórios seguintes do mesmo jogo entram nela. A ponte guarda qual postagem é de qual jogo num Durable Object por jogo (`GameThreads`, com SQLite, disponível no plano gratuito). Ele é criado sozinho no deploy, não há nada a fazer. Os relatórios do mesmo jogo são atendidos em fila, então dois envios simultâneos não abrem postagens duplicadas.
4. **Segredo:** rode o comando abaixo e cole a URL do webhook quando ele pedir:
   ```
   bunx wrangler secret put DISCORD_WEBHOOK
   ```
5. **Publicar:**
   ```
   bunx wrangler deploy
   ```
   O comando mostra o endereço da ponte, como `https://peligames-report.<sua-conta>.workers.dev`.
6. **Ligar ao PeliGames:** compile com `PELIGAMES_REPORT_RELAY=https://peligames-report.<sua-conta>.workers.dev` no ambiente, ou ponha esse endereço em `DEFAULT_REPORT_RELAY` (`src-tauri/src/commands/nexus_reports.rs`). O endereço da ponte não é segredo.

**Tag opcional:** para marcar as postagens com uma tag do fórum, descomente `FORUM_TAG_ID` em `wrangler.toml` com o ID da tag. Para obter o ID, ative o modo de desenvolvedor do Discord e use *Copiar ID* na tag. Depois, rode `bunx wrangler deploy` de novo.

## Se o webhook vazar ou for abusado

1. Apague o webhook no Discord e crie outro.
2. Rode `bunx wrangler secret put DISCORD_WEBHOOK` com a URL nova.

O PeliGames não precisa de versão nova para isso.
