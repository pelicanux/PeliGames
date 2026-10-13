# Login Discord do PeliGames

Serviço separado de `tools/report-relay`. Não altera nem usa o webhook dos relatórios.
O launcher abre o navegador externo e consulta o resultado com uma credencial aleatória;
o código OAuth, o Client Secret e os tokens Discord ficam apenas neste serviço.

## Configuração pelo responsável pelo canal

### Assistente no terminal

Na raiz do projeto, execute:

```bash
bash tools/discord-auth/setup.sh
```

O assistente verifica a conta Cloudflare, oferece login pelo navegador, solicita o
Client Secret pelo prompt oculto do Wrangler e publica **peligames-discord-auth**.
Ele não altera o serviço de relatórios e não salva nem recebe o segredo como argumento.
No final, cole a URL pública mostrada pelo deploy: ele monta o endereço `/callback`
para cadastrar em **Discord Developer Portal → OAuth2 → Redirects**.
Envie somente essa URL pública para configurar o build do launcher.

### Passos manuais

1. Em https://discord.com/developers/applications, use **New Application**, nome PeliGames.
2. Copie **Application ID** de General Information para `DISCORD_CLIENT_ID` em `wrangler.toml`.
3. Confirme `GUILD_ID`: o valor inicial veio do serviço de relatórios existente.
4. No Discord, ative Configurações do usuário → Avançado → Modo desenvolvedor.
   Copie o ID do servidor e, em Configurações do servidor → Cargos, o ID do cargo
   gerenciado de membros do YouTube. Configure `YOUTUBE_ROLE_ID`.
   Opcionalmente configure `BOOSTER_ROLE_ID`; o campo `premium_since` também identifica
   quem impulsiona este servidor. Nitro sozinho não dá acesso.
5. Autentique o Wrangler em sua conta Cloudflare e cadastre o segredo diretamente:
   `bunx wrangler secret put DISCORD_CLIENT_SECRET` (execute nesta pasta).
   Obtenha o Client Secret em OAuth2 no Discord Developer Portal. Não envie esse valor
   em chats, não o coloque no código nem em variáveis VITE_. Não é necessário token de bot.
6. Publique com `bunx wrangler deploy` e anote a origem HTTPS retornada.
7. Em Discord Developer Portal → OAuth2 → Redirects, cadastre exatamente
   `https://SEU-WORKER.workers.dev/callback`. Salve as alterações.
8. O launcher usa por padrão `https://peligames-discord-auth.nairomarttins97.workers.dev`.
   Para outra origem, compile com `PELIGAMES_DISCORD_RELAY=https://SEU-WORKER.workers.dev`.
   Essa variável é pública e lida em tempo de compilação; recompilar é necessário.

Se quiser usar outra origem/domínio, cadastre sua `/callback` exata no Discord.
Apenas `identify` e `guilds.members.read` são solicitados: sem email, mensagens ou
entrada automática no servidor. Os membros devem entrar no servidor e conectar
YouTube ao Discord para receber o cargo gerenciado pela integração oficial.

## Fluxo

- `/start`: gera sessão de login de dez minutos e segredo que só o launcher recebe.
- `/authorize`: abre autorização Discord e grava cookie HttpOnly/Secure/SameSite=Lax.
- `/callback`: valida state e cookie, consome o pedido uma vez, troca código por tokens.
- `/status`: exige segredo; consulta usuário e cargos; retorna elegibilidade YouTube OU boost.
- `/disconnect`: revoga o refresh token Discord e remove a sessão.
- Sessões sem atividade expiram após 90 dias. Tokens e segredos não aparecem na resposta
  pública nem em logs. Sessão do launcher é guardada via libsecret/secret-tool, como Nexus.

Na aba Conexões, o launcher verifica ao abrir e permite verificar novamente.
Durante o login, consulta a cada três segundos, por no máximo dez minutos.
Sem rede, mostra a última verificação salva, claramente identificada como offline.

## Efeitos exclusivos e uso offline

Suave e Desabilitado ficam livres. Os demais efeitos exigem uma autorização assinada
ECDSA P-256, válida por sete dias, de membro YouTube, impulsionador ou `OWNER_USER_ID`.
A chave privada fica no segredo `ENTITLEMENT_SIGNING_KEY`; somente a chave pública
fica em `src/theme/discordEntitlementKey.json`, podendo ser versionada.

Para gerar a chave de assinatura e publicar esta atualização, execute na raiz:

```bash
bash tools/discord-auth/publish-benefits.sh
```

O assistente gera a chave em memória, envia a parte privada por stdin ao Wrangler e
salva apenas a chave pública no launcher. Se ela já existe, mantém a chave existente.
Não apague o segredo de assinatura do Worker sem preparar uma rotação de chaves.
Recompile o launcher depois de gerar a chave pública. Sem chave pública válida,
os efeitos exclusivos ficam bloqueados. Não é necessário mudar a URL `/callback`.

O launcher verifica assinatura, usuário, sessão, servidor, finalidade e vencimento.
Ao iniciar, após mudanças no login e a cada seis horas, tenta renovar a verificação.
Sem rede, a autorização anterior segue válida até vencer. Desconectar bloqueia
imediatamente os extras; um efeito exclusivo salvo passa a usar Suave, preservando
sua preferência para quando o acesso for restaurado. Essa verificação não impede
modificações intencionais do código local nem garante resistência a manipulações
do relógio/sistema operacional pelo proprietário do computador.

A preview usa a conexão simulada desconectada, mostrando os extras bloqueados.
O teste real de login requer o launcher nativo recompilado.

## Verificação local

`bun test tools/discord-auth/src/index.test.js src/theme/discordBenefits.test.ts` a partir da raiz do projeto.

Documentação: https://docs.discord.com/developers/topics/oauth2
