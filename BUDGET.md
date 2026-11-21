# BUDGET — orçamento de memória do schematize-updater

Piso de eficiência de runtime da casa (`eficiencia-runtime.md` §1). O teto vale: o teste
`tests/orcamento_rss.rs` falha acima dele. Subir o teto exige medição nova e justificativa aqui.

Binário: `schematize-updater` (CLI, std-only, ZERO dependências; release 522 KB com o perfil
compartilhado). Teto de CLI da casa: 256 MiB; este fica muito abaixo.

| Operação | RSS pico alvo (teto) | Medido antes | Medido depois | Como medir |
|---|---|---|---|---|
| `version` | 8 MiB | ~3,2 MiB | ~3,2 MiB | `cargo test --test orcamento_rss -- --nocapture` |
| `help` | 8 MiB | ~0,5 MiB | ~0,5 MiB | idem |
| `status` (curl + filhos de versão) | 8 MiB | ~2,8 MiB | ~2,8 MiB | idem |
| `install`/`update` | fora do gate (rede + compilação do app) | n/d | n/d | filhos (curl/compilador) têm RSS próprio, fora deste processo |

Notas de método: o teste polla `VmHWM` de `/proc/<pid>/status` a cada 1 ms até o processo sair
(std-only). Processo muito curto pode escapar entre duas leituras, então o valor é um piso do
pico real; o teto de 8 MiB (~2,5x o medido) cobre isso. Sem `/proc` (macOS/Windows) o teste
imprime `PULADO` explicitamente. Referência manual: `ru_maxrss` do filho.

Limites por construção: nenhuma dependência; leitura de rede só via `curl` com
`--max-filesize 2097152` (teste `curl_de_texto_leva_teto_e_timeout`), então nenhuma resposta
infla a `String` do updater além de 2 MiB. Sem canais, caches nem threads próprias.

Causa raiz do baixo consumo: não há o que cortar em dependências (zero) nem em leitura (só
arquivos pequenos de config e o stdout do curl). A mudança foi preventiva (teto de corpo).
