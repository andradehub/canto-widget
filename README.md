# Canto · Rust

Música e bateria de mouses compatíveis na barra de tarefas ou em um widget flutuante, com interface nativa do Windows. A detecção usa dados do Windows, o padrão HID e protocolos específicos de fabricantes.

## Usar

[**Baixar CantoWidget.exe para Windows**](https://github.com/andradehub/canto-widget/releases/latest/download/CantoWidget.exe)

Baixe o `.exe` e dê dois cliques para abrir. É portátil: não precisa de instalação, .NET, Rust ou navegador embutido. Não é necessário baixar o código-fonte nem compilar.

O [pacote ZIP](https://github.com/andradehub/canto-widget/releases/latest/download/CantoWidget-Windows.zip) também está disponível, com documentação e licenças; nele, abra `publish/CantoWidget.exe`.

A seta do widget e o botão direito abrem as opções:

- **Exibição → Player e bateria:** mostra capa, faixa, artista, controles e bateria.
- **Exibição → Somente bateria:** esconde o player e reduz o widget para 100 px na escala de 100%.
- **Ocultar player quando não houver mídia:** mostra só a bateria quando o Windows não tem uma sessão de mídia; o player reaparece quando uma sessão fica disponível. Uma faixa pausada continua disponível para dar play.
- **Posição → Integrado à barra de tarefas:** encaixa o widget antes da área do relógio, com espaço reservado entre os botões dos outros aplicativos.
- **Posição → Flutuante:** solta o widget, com cantos arredondados e contorno discreto; arraste a área da música ou outra área sem botão para reposicionar.
- **Cores → Automático:** fundo transparente na barra de tarefas; no modo flutuante, usa vidro com blur e tonalidade discreta.
- **Cores → Desfoque no modo flutuante:** liga ou desliga o blur. A cor escolhida para o fundo também pode ser usada como tonalidade do vidro.
- **Cores → Fundo totalmente transparente:** remove o fundo nos dois modos e desliga o blur. A área transparente continua recebendo cliques e pode ser arrastada no modo flutuante.
- **Cores:** cor personalizada do fundo, cor dos ícones e cor do progresso, com seletor nativo do Windows e opção de restaurar as cores padrão.
- **Iniciar com o Windows** e atualização manual da bateria.

As preferências são aplicadas na hora e salvas em `%LOCALAPPDATA%/CantoWidget/settings.json`. As configurações são locais. O modo integrado foi validado na barra horizontal do Windows 10 x64; a barra moderna do Windows 11 não oferece o mesmo encaixe.

O fundo automático usa composição nativa por pixel ([UpdateLayeredWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow)), revelando o fundo real da barra, inclusive textura e variações de cor. Uma opacidade de 1/255 preserva os cliques nas áreas vazias. Ícones, capa e linha de progresso permanecem visíveis. Escolher uma cor de fundo aplica um preenchimento sólido na barra e uma tonalidade no vidro flutuante quando o blur está ligado.

O blur é composto pelo Windows, sem capturas de tela ou filtros contínuos no aplicativo. Foi validado no Windows 10 22H2. A política de accent usa [SetWindowCompositionAttribute](https://learn.microsoft.com/en-us/windows/win32/dwm/setwindowcompositionattribute), resolvido dinamicamente; seus valores de accent não são um contrato público estável. Se a chamada não estiver disponível ou for recusada, o flutuante conserva um fundo sólido legível.

## Consumo

O aplicativo foi reescrito em Rust, com Win32/GDI para desenhar texto, capa e ícones diretamente. Não usa WPF, renderização contínua de bitmaps, WebView, Electron, Tauri ou processo do PowerShell residente.

A capa só é decodificada quando muda. O desenho nativo é leve e reaproveita fontes e miniaturas em cache. No modo somente bateria, a consulta de mídia é suspensa e as referências do player/capa são liberadas. O mouse é consultado a cada 30 segundos.

A versão Rust foi observada em aproximadamente 15–30 MiB de memória residente. São amostras de Working Set, que incluem páginas compartilhadas; o consumo varia com player, capa e interação. Não foi aplicada redução artificial do Working Set.

O executável Rust tem aproximadamente 0,8 MiB; a distribuição não inclui o runtime .NET de cerca de 198 MB do executável anterior.

## Música

Lê as sessões de mídia expostas pelo Windows através de Global System Media Transport Controls. Prioriza uma sessão em reprodução; se não existir, usa a sessão atual. Spotify, Edge e outros players precisam disponibilizar essa integração para mostrar os metadados. Capa e comandos dependem do que o player oferece. Não acessa sua conta de streaming nem pede senha.

## Bateria do mouse

O aplicativo procura automaticamente um mouse com uma leitura válida de bateria. O nome detectado e o estado aparecem no menu. Não é necessário editar o código para usar outro modelo que informe a bateria por um dos caminhos implementados:

| Caminho | Dispositivos atendidos | Validação |
| --- | --- | --- |
| Windows | Mouses cuja bateria é disponibilizada pelo sistema/driver, incluindo dispositivos Bluetooth compatíveis | Leitura de propriedades implementada; outros modelos precisam de validação física |
| HID padrão | Mouses de qualquer marca que exponham `Battery Strength` em um relatório Feature ou Input da coleção de mouse | Interpretação e escala testadas; aguardando validação em hardware compatível |
| AJAZZ/CompX | Receptor `249A:5C2F`, `248A:5C2F` e variantes cabeadas `248A:5C2E/5D2E/5E2E` reconhecidas | AJ139 Pro por receptor `249A:5C2F` validado fisicamente; outras variantes ainda não |

Conectar por USB ou Bluetooth, por si só, não garante que um mouse forneça sua bateria. Receptores com protocolo proprietário precisam de suporte específico. Um mouse com fio sem bateria não tem percentual a mostrar; um modelo recarregável ligado pelo cabo pode informar a bateria se seu protocolo for compatível. Quando não existe leitura válida, aparece “—”, sem inventar um percentual. Havendo vários mouses, é usada a primeira leitura disponível, priorizando o protocolo específico já reconhecido.

As consultas não alteram DPI, botões, iluminação ou firmware. O caminho HID padrão só consulta relatórios declarados de bateria, sem consumir os eventos de movimento. A escala segue o intervalo lógico informado pelo dispositivo; valores fora dele são rejeitados. A leitura do Windows usa propriedades de bateria dos nós do mouse e de seus pais; a propriedade do provedor Bluetooth pode variar entre versões do Windows.

No protocolo AJAZZ/CompX, a consulta `0x10` e os anúncios `0xC0` são validados por formato e faixa de percentual; a identificação exige checksum correto. Se o mesmo dispositivo parar de responder, a última leitura pode ser conservada por até três minutos, marcada no estado. Ao remover ou trocar o dispositivo, a leitura anterior não é reaproveitada para outro mouse. Acorde o mouse ou use **Atualizar bateria agora**.

Diagnóstico local, sem abrir outra instância da interface:

```powershell
./publish/CantoWidget.exe --diagnostics "$PWD/diagnostics.json"
```

O diagnóstico inclui a leitura selecionada e o resultado dos caminhos padrão Windows/HID, para ajudar a verificar outros modelos. É um arquivo local que pode conter identificadores do dispositivo.

## Compilar e testar

Com Rust estável e a cadeia de compilação do Windows instalada:

```powershell
cargo test --release --locked
./scripts/Build.ps1
```

O caminho habitual no Windows é a toolchain MSVC com as ferramentas C++/SDK indicadas no [guia oficial de Rust para Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup). O código também foi compilado e validado com a toolchain GNU/LLD; nessa toolchain, `windres` precisa estar no PATH ou ser indicado pela variável `WINDRES`. O manifesto de compatibilidade é incorporado ao executável durante a compilação.

Os testes Rust cobrem o pacote real do AJ139 Pro, checksum, limites e escalas de bateria, modos de exibição, migração/persistência de preferências, cores e composição da interface.

A versão anterior em C#/WPF permanece no histórico e na release 1.0. A versão 2 é executada em Rust, incluindo interface, mídia, bateria e preferências.
