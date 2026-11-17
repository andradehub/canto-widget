# Canto

Sua música e a bateria do mouse, no canto direito do Windows.

Widget nativo em C#/WPF, inspirado em uma faixa compacta azul escura. Mostra capa, faixa, artista, controles de reprodução e bateria do AJAZZ AJ139 Pro. Fica acima da barra de tarefas, sempre visível, sem botão na barra. Arraste para reposicionar; a seta abre as opções. O ícone da bandeja permite mostrar o widget ou sair.

## Executar

Abra `publish/CantoWidget.exe`. A distribuição portátil inclui o .NET: o usuário não precisa instalar o SDK nem executar como administrador. Windows 10 2004 ou mais recente, Windows 11, x64.

Nas opções você pode restaurar o canto direito, iniciar com o Windows e pausar a sincronização. Posição e preferências ficam somente em `%LOCALAPPDATA%/CantoWidget/`.

## Música

Lê as sessões de mídia do próprio Windows via Global System Media Transport Controls. Prioriza uma sessão que esteja tocando; se nenhuma estiver, usa o player atual. Funciona com os players que disponibilizam essa integração, como Spotify e navegadores compatíveis. A capa e os botões dependem do que o player expõe ao Windows. Se nenhuma sessão existir, mostra “Nada tocando agora”. Não consulta sua conta de streaming nem pede senha.

## Mouse: AJAZZ AJ139 Pro

Consulta a interface de status HID do receptor `249A:5C2F`, interface `MI_02`, e do cabo `248A:5D2E`. Faz a consulta `0x10`, valida formato, checksum e percentual antes de mostrar. Também reconhece anúncios `0xC0`. Não altera DPI, perfis, botões, iluminação ou firmware.

O adaptador usa o protocolo CompX documentado para essa família de receptores. Também aceita as variantes `248A:5C2F` (receptor) e `248A:5C2E/5D2E/5E2E` (cabo), listadas no driver oficial para o AJ139 Pro. IDs USB podem ser compartilhados entre modelos: a confirmação no seu AJ139 Pro depende de uma resposta válida do próprio firmware. Outras revisões e outros IDs não estão habilitados. Sem resposta, o widget mostra “—”; se houver uma leitura anterior, conserva-a por até três minutos com a indicação de última leitura. Acorde o mouse se ele estiver dormindo. A consulta ocorre a cada 30 segundos; “Atualizar bateria agora” permite consultar manualmente. O indicador de carga só é aceito no modo cabeado.

Para registrar um diagnóstico local:

```powershell
./publish/CantoWidget.exe --diagnostics "$PWD/diagnostics.json"
```

Diagnósticos não são enviados ao GitHub pela sincronização automática. Logs locais: `%LOCALAPPDATA%/CantoWidget/canto.log`.

## GitHub automático

Repositório de destino: `andradehub/canto-widget`, público. Se ainda não estiver conectado, use “Conectar e publicar no GitHub” nas opções. O assistente usa o GitHub CLI oficial e pede a autenticação no navegador. Ele verifica a conta, cria o repositório público e envia o código. Também pode ser executado com `./scripts/Connect-GitHub.ps1`.

Depois de autenticar o GitHub e configurar `origin`, o widget inicia o sincronizador enquanto estiver aberto. Após 20 segundos sem alterações, ele cria um commit com data e hora e faz push para a branch atual. Em uma falha de rede ou login, mantém os commits locais e tenta novamente a cada minuto. A seta do widget mostra o estado do envio. Ao pausar ou sair, deixa uma operação Git já iniciada terminar antes de encerrar o sincronizador.

Ele sincroniza código em `src/`, scripts, testes, workflow e documentação listados explicitamente no script. Binários, configurações locais, `.env`, chaves e credenciais não entram no envio automático. Arquivos novos de outros tipos precisam ser revisados e incluídos manualmente. Se houver arquivos fora dessa lista já no índice do Git, o sincronizador para e solicita revisão. Nunca executa force push, reset ou merge automático. Se alguém editar a mesma branch remotamente, pause a sincronização, integre a mudança com Git e retome.

Para usar em outro computador, instale Git e GitHub CLI e autentique sua própria conta:

```powershell
gh auth login --hostname github.com --git-protocol https --web
gh auth setup-git
git clone https://github.com/andradehub/canto-widget.git
cd canto-widget
git config user.name "Seu nome"
git config user.email "Seu e-mail público ou noreply do GitHub"
```

Mantenha `publish/` dentro da pasta do projeto para o widget localizar o sincronizador. Se você copiar apenas o executável, o widget funciona, mas o código-fonte não é sincronizado. As preferências do aplicativo são locais e não são versionadas. O sincronizador vale para este projeto; outros projetos precisam de sua própria configuração.

## Compilar e verificar

Com .NET SDK 10:

```powershell
dotnet run --project tests/ProtocolTests.csproj -c Release
./tests/Sync.Tests.ps1
./scripts/Build.ps1
```

O GitHub Actions executa os testes, compila o aplicativo e disponibiliza `CantoWidget-Windows-x64` como artefato de cada execução. O workflow não publica credenciais nem binários no histórico Git.

Os testes do protocolo cobrem respostas válidas, carga, report ID, checksum incorreto, resposta vazia e limites de percentual. Os testes de integração usam repositórios locais descartáveis para verificar commit, push, exclusão de credenciais e rejeição de divergência remota.

Teste adicional de mídia em uma sessão interativa do Windows (não é executado no CI): `dotnet run --project tests/MediaSmoke/MediaSmoke.csproj -c Release`. Ele abre uma faixa silenciosa de teste, verifica os metadados e envia pausa pela API. Execute sem outro player tocando.

Referências técnicas e créditos em `THIRD_PARTY_NOTICES.md`.
