# MyMemory — serviço externo de tradução

MyMemory é um serviço de memória de tradução da **Translated S.r.l.** O PeliGames utiliza sua API HTTPS, sob demanda, para traduzir descrições de mods do inglês para português brasileiro. Nenhum motor, modelo, banco de traduções ou código do MyMemory é distribuído dentro do launcher. Este crédito não atribui ao serviço a licença GPL do PeliGames e não substitui os termos de uso do fornecedor.

## Dados enviados

Ao clicar em **Traduzir descrição**, o texto da descrição é enviado a `api.mymemory.translated.net`, junto do par de idiomas `en|pt-BR`. A implementação não envia a chave Nexus, tokens de conta, e-mail, caminhos dos jogos ou arquivos locais. O serviço recebe o IP da conexão e pode armazenar e processar os segmentos conforme sua política. O PeliGames utiliza apenas o endpoint `/get`; não chama `/set` nem importa memórias de tradução.

A interface identifica o fornecedor como **Tradução online via MyMemory**. O original permanece disponível. O recurso não funciona offline e sua disponibilidade e qualidade dependem do fornecedor.

## Termos e limites consultados em 16/10/2026

- [Termos de serviço e privacidade](https://mymemory.translated.net/terms-and-conditions): permitem acesso automatizado pela API dentro das restrições publicadas; proíbem tráfego que prejudique o serviço, contornar limites, tradução de mais de um parágrafo por consulta e revenda do serviço como tal sem consentimento. Informam armazenamento dos segmentos e possível processamento por parceiros.
- [Especificação da API](https://mymemory.translated.net/doc/spec.php): limite de **500 bytes UTF-8** no parâmetro `q`.
- [Limites de uso](https://mymemory.translated.net/doc/usagelimits.php): uso anônimo de até **5.000 caracteres/dia**; com e-mail válido no parâmetro `de`, até **50.000 caracteres/dia**. O PeliGames não envia esse e-mail atualmente e não deve incluir e-mail de usuário ou desenvolvedor sem uma decisão explícita.

Não foi identificada, nas páginas consultadas, uma cláusula específica exigindo um aviso de copyright de software para o simples consumo da API. A atribuição acima documenta a integração; ela não certifica autorização para outros usos nem o cumprimento de todas as obrigações do serviço. Os termos e limites podem mudar. As descrições dos mods continuam sujeitas aos direitos de seus autores.
