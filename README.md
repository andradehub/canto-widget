# Canto · Rust

Música e bateria do AJAZZ AJ139 Pro na barra de tarefas ou em um widget flutuante, com interface nativa do Windows.

## Usar

Extraia o pacote e abra `publish/CantoWidget.exe`. Não precisa instalar .NET, Rust, navegador embutido ou outro runtime para executar.

A seta do widget e o botão direito abrem as opções:

- **Exibição → Player e bateria:** mostra capa, faixa, artista, controles e bateria.
- **Exibição → Somente bateria:** esconde o player e reduz o widget para 100 px na escala de 100%.
- **Ocultar player quando não houver mídia:** mostra só a bateria quando o Windows não tem uma sessão de mídia; o player reaparece quando uma sessão fica disponível. Uma faixa pausada continua disponível para dar play.
- **Posição → Integrado à barra de tarefas:** encaixa o widget antes da área do relógio, com espaço reservado entre os botões dos outros aplicativos.
- **Posição → Flutuante:** solta o widget; arraste a área da música ou outra área sem botão para reposicionar.
- **Cores:** fundo automático, cor personalizada do fundo, cor dos ícones e cor do progresso, com seletor nativo do Windows e opção de restaurar as cores padrão.
- **Iniciar com o Windows**, atualização manual da bateria e pausa da sincronização do código.

As preferências são aplicadas na hora e salvas em `%LOCALAPPDATA%/CantoWidget/settings.json`. São locais, sem publicação no GitHub. O modo integrado foi validado na barra horizontal do Windows 10 x64; a barra moderna do Windows 11 não oferece o mesmo encaixe.

## Consumo

O aplicativo foi reescrito em Rust, com Win32/GDI para desenhar texto, capa e ícones diretamente. Não usa WPF, renderização contínua de bitmaps, WebView, Electron, Tauri ou processo do PowerShell residente.

A capa só é decodificada quando muda. O desenho nativo é leve e reaproveita fontes e miniaturas em cache. No modo somente bateria, a consulta de mídia é suspensa e as referências do player/capa são liberadas. O mouse é consultado a cada 30 segundos. O monitor de código verifica metadados dos arquivos a cada 5 segundos e só executa o Git quando precisa enviar alterações.

Na máquina de desenvolvimento, a versão anterior usava cerca de 147 MiB de memória residente no aplicativo e 67,5 MiB no auxiliar de sincronização. A versão Rust foi observada em aproximadamente 16–30 MiB, com a sincronização no próprio processo. São amostras de Working Set, que incluem páginas compartilhadas; o consumo varia com player, capa e interação. Não foi aplicada redução artificial do Working Set.

O executável Rust tem aproximadamente 0,8 MiB; a distribuição não inclui o runtime .NET de cerca de 198 MB do executável anterior.

## Música

Lê as sessões de mídia expostas pelo Windows através de Global System Media Transport Controls. Prioriza uma sessão em reprodução; se não existir, usa a sessão atual. Spotify, Edge e outros players precisam disponibilizar essa integração para mostrar os metadados. Capa e comandos dependem do que o player oferece. Não acessa sua conta de streaming nem pede senha.

## Bateria AJAZZ AJ139 Pro

Leitura de status pela interface HID `MI_02`, sem alterar DPI, botões, iluminação ou firmware. Receptor `249A:5C2F` validado fisicamente com identidade `M129`. Também reconhece as variantes `248A:5C2F` (receptor) e `248A:5C2E/5D2E/5E2E` (cabo) listadas para a família; o modo cabeado ainda precisa de validação física.

A consulta `0x10` e os anúncios `0xC0` são validados por formato e faixa de percentual; a identificação exige checksum correto. Se o mouse parar de responder, a última leitura é conservada por até três minutos e marcada no estado. Depois disso aparece “—”. Acorde o mouse ou use **Atualizar bateria agora**.

Diagnóstico local, sem abrir outra instância da interface:

```powershell
./publish/CantoWidget.exe --diagnostics "$PWD/diagnostics.json"
```

O diagnóstico não é enviado ao GitHub pela sincronização automática.

## GitHub automático

Destino deste projeto: [andradehub/canto-widget](https://github.com/andradehub/canto-widget), público.

Com o aplicativo aberto e autenticado, após 20 segundos sem novos salvamentos ele faz commit e push do código na branch atual. Em falha, mantém os commits locais e tenta novamente. A sincronização ocorre em uma thread Rust; o PowerShell só é usado pelo assistente de conexão quando você escolhe essa opção.

O monitor envia os arquivos de código Rust, Cargo.toml/Cargo.lock, scripts, testes e documentação permitidos explicitamente. Não envia binários, `.env`, configurações pessoais, diagnósticos ou credenciais. Arquivos fora da lista já preparados no índice bloqueiam o envio para revisão. Não executa force push, reset ou merge automático. Alterações remotas divergentes precisam ser integradas manualmente.

Mantenha `publish/` dentro da pasta do projeto para sincronizar o código. Um executável copiado isoladamente funciona como widget, mas não encontra o repositório. Outros projetos precisam de sua própria configuração.

Para conectar, use **Conectar e publicar no GitHub** no menu ou `./scripts/Connect-GitHub.ps1`. O assistente verifica a conta, cria o repositório público se necessário e configura o acesso com GitHub CLI. Nenhum token é gravado no código-fonte.

## Compilar e testar

Com Rust estável e a cadeia de compilação do Windows instalada:

```powershell
cargo test --release --locked
./scripts/Build.ps1
```

O caminho habitual no Windows é a toolchain MSVC com as ferramentas C++/SDK indicadas no [guia oficial de Rust para Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup). O código também foi compilado e validado com a toolchain GNU/LLD.

Os testes Rust cobrem o pacote real do AJ139 Pro, checksum, limites, modos de exibição, migração/persistência de preferências, cores e integração Git com repositórios locais descartáveis. O teste Git verifica que credenciais ficam fora do commit e que o histórico é preservado. `tests/Sync.Tests.ps1` verifica o assistente de sincronização manual em PowerShell.

A versão anterior em C#/WPF permanece no histórico e na release 1.0. A versão 2 é executada em Rust, incluindo interface, mídia, bateria, preferências e sincronização.