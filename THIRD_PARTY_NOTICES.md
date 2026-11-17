# Referências e licenças

Implementação própria em Rust/Win32. Nenhum código-fonte dos projetos de protocolo foi incorporado; foram usados fatos documentados das capturas de status.

- Microsoft: [Global System Media Transport Controls](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager).
- Microsoft: [Win32](https://learn.microsoft.com/en-us/windows/win32/) e [Windows bindings for Rust](https://github.com/microsoft/windows-rs).
- Design: [materiais em aplicativos Windows](https://learn.microsoft.com/en-us/windows/apps/develop/ui/materials), com superfícies discretas e destaque de progresso.
- Protocolo AJAZZ/CompX: [Ajazzy](https://github.com/Geovane2dd/Ajazzy/blob/main/docs/reverse-engineering.md), [HaloBattery issue #74](https://github.com/HeyOkay/HaloBattery/issues/74) e [johan-akn/aj179-linux](https://github.com/johan-akn/aj179-linux).

Dependências: Rust standard library, windows/windows-sys e dependências, serde/serde_json, sha2 e image (PNG/JPEG). As versões estão fixadas no Cargo.lock. O pacote inclui os avisos e licenças das bibliotecas distribuídas e a exceção de runtime do GCC usada pela compilação GNU.

AJAZZ, Windows e GitHub pertencem a seus respectivos titulares; o projeto não é afiliado a eles. Capas e metadados vêm do player local e não são distribuídos no repositório.