#!/usr/bin/env python3
"""
Gerador do Relatório de Auditoria de Segurança — Screen Share.

Uso (a partir da raiz do repositório):

    docs/security-audit/.venv/bin/python docs/security-audit/generate_report.py

Dependências (ambiente isolado, nunca global):

    python3 -m venv docs/security-audit/.venv
    docs/security-audit/.venv/bin/pip install reportlab matplotlib

Saída: docs/security-audit/relatorio-auditoria-seguranca.pdf
       docs/security-audit/_charts/  (PNGs intermediários dos gráficos)
"""

from __future__ import annotations

import base64
import html as _html
import os
import re
import textwrap
from datetime import date

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_JUSTIFY, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import cm, mm
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    HRFlowable,
    Image,
    NextPageTemplate,
    PageBreak,
    PageTemplate,
    Paragraph,
    Preformatted,
    Spacer,
    Table,
    TableStyle,
)

HERE = os.path.dirname(os.path.abspath(__file__))
CHART_DIR = os.path.join(HERE, "_charts")
OUT_PDF = os.path.join(HERE, "relatorio-auditoria-seguranca.pdf")
OUT_HTML = os.path.join(HERE, "relatorio-auditoria-seguranca.html")
# Conteúdo-apenas (sem <!doctype>/<html>/<head>/<body>) para publicar como Artifact.
OUT_ARTIFACT = os.path.join(HERE, "artefato.html")

REPORT_NAME = "Relatório de Auditoria de Segurança — Screen Share"
AUDIT_DATE = date.today().strftime("%d/%m/%Y")

# ---------------------------------------------------------------------------
# Paleta
# ---------------------------------------------------------------------------
SEV_COLOR = {
    "CRÍTICA": "#B91C1C",
    "ALTA": "#EA580C",
    "MÉDIA": "#D97706",
    "BAIXA": "#2563EB",
    "INFORMATIVA": "#6B7280",
    "PONTO FORTE": "#059669",
}
SEV_ORDER = ["CRÍTICA", "ALTA", "MÉDIA", "BAIXA", "INFORMATIVA"]

# ---------------------------------------------------------------------------
# Dados da auditoria
# ---------------------------------------------------------------------------
# Categorias usadas no gráfico de barras e no agrupamento da tabela.
CATEGORIES = [
    "1 · Isolamento de inquilino/dono",
    "2 · Permissão definida no navegador",
    "3 · IDOR / BOLA",
    "4 · Chaves expostas (segredos)",
    "5 · Entrada não tratada (XSS/spoofing)",
    "6 · DoS / exaustão de recursos",
    "7 · Sinalização WebSocket / WebRTC",
    "8 · TURN / infraestrutura",
    "9 · Aplicação desktop (Electron)",
    "10 · Hardening HTTP",
]

FINDINGS = [
    dict(
        id="F01",
        sev="CRÍTICA",
        cat="8 · TURN / infraestrutura",
        title="coturn sem allowlist de peer-IP e sem cotas de banda/alocação",
        loc=[
            "docker-entrypoint.sh:24-37",
            "fly.toml:23-25",
            "crates/signaling/src/turn.rs:16",
        ],
        code=(
            "turnserver \\\n"
            "  --no-cli --fingerprint --use-auth-secret \\\n"
            '  --static-auth-secret=\"$TURN_SECRET\" \\\n'
            '  --realm=\"${TURN_REALM:-screenshare}\" \\\n'
            "  --listening-port=3478 \\\n"
            "  --min-port=... --max-port=... \\\n"
            '  --external-ip=\"$TURN_EXTERNAL_IP\" \\\n'
            "  --log-file=stdout --no-tls --no-dtls &\n"
            "# sem --denied-peer-ip / --allowed-peer-ip\n"
            "# sem --total-quota / --user-quota / --max-bps / --bps-capacity"
        ),
        why=(
            "O relay exige credencial (`--use-auth-secret`), mas qualquer participante de "
            "qualquer sala recebe uma credencial TURN válida por 6 h no snapshot `Joined` "
            "(`ServerMessage::Joined.turn`). Para uma sala pública, isso é qualquer pessoa "
            "que conheça ou adivinhe o código. De posse da credencial, um atacante pode "
            "pedir alocações e mandar o coturn encaminhar pacotes para endereços "
            "arbitrários — inclusive `169.254.169.254` (metadados de nuvem), faixas "
            "RFC1918/`fdaa::/16` (rede privada 6PN da organização na Fly) e `127.0.0.0/8`. "
            "coturn recente nega loopback por padrão, mas NÃO nega link-local nem RFC1918. "
            "Sem `--total-quota`/`--max-bps` o mesmo relay serve de amplificador de tráfego "
            "às custas da banda/conta da Fly."
        ),
        impact=(
            "SSRF a partir da infraestrutura de relay: varredura e acesso a serviços "
            "internos da Fly e ao endpoint de metadados (potencial roubo de tokens do "
            "ambiente). Abuso de banda / negação de serviço por exaustão de alocações."
        ),
        fix=(
            "Adicionar `--denied-peer-ip=0.0.0.0-0.255.255.255`, `10.0.0.0-10.255.255.255`, "
            "`100.64.0.0-100.127.255.255`, `169.254.0.0-169.254.255.255`, "
            "`172.16.0.0-172.31.255.255`, `192.168.0.0-192.168.255.255`, `::1`, `fc00::/7`, "
            "`fe80::/10` (e liberar só o necessário com `--allowed-peer-ip`). "
            "Definir `--total-quota`, `--user-quota`, `--max-bps`, `--bps-capacity`, "
            "`--no-multicast-peers`. Reduzir `CREDENTIAL_TTL` para ~1 h."
        ),
        accept=[
            "`turnutils_peer` / `turnutils_uclient` contra 169.254.169.254 e um IP RFC1918 falham (peer negado).",
            "Alocações acima da cota são rejeitadas; `--max-bps` visível em `turnserver -h` do processo em execução.",
            "Teste de fumaça de sala real (2 abas, mídia P2P) continua passando.",
            "`CREDENTIAL_TTL` documentado com o novo valor e razão.",
        ],
    ),
    dict(
        id="F02",
        sev="ALTA",
        cat="6 · DoS / exaustão de recursos",
        title="Servidor de sinalização sem limites (salas globais, conexões, tamanho/taxa de mensagem, canal ilimitado)",
        loc=[
            "crates/signaling/src/ws.rs:36-66",
            "crates/signaling/src/registry.rs:90-160",
            "apps/web/src/main.rs:39-55",
        ],
        code=(
            "let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();  // ws.rs:43\n"
            "while let Some(Ok(msg)) = ws_receiver.next().await {           // ws.rs:62\n"
            "    let Message::Text(text) = msg else { continue };\n"
            "    let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) else { continue };\n"
            "    // sem limite de tamanho, sem rate limit, sem timeout de inatividade\n"
            "// registry.rs: HashMap<String, Room> sem MAX_ROOMS nem limite por IP"
        ),
        why=(
            "Nenhum limite global de salas, de conexões WebSocket simultâneas, de mensagens "
            "por segundo, nem `idle timeout`/heartbeat. O canal de saída é "
            "`mpsc::unbounded_channel`: uma `sdp`/`candidate` de dezenas de MB "
            "(limite default do axum-ws ~64 MiB) repassada a um receptor lento acumula sem "
            "teto na fila dele. Um único socket sem autenticação prévia pode criar salas, "
            "abrir milhares de conexões ociosas (slowloris) ou inundar sinalização. A VM de "
            "produção tem 256 MB e `min_machines_running = 0`."
        ),
        impact=(
            "Negação de serviço remota, não autenticada e trivial (esgotamento de memória / "
            "descritores) derrubando todo o serviço."
        ),
        fix=(
            "Definir consts nomeadas: `MAX_ROOMS`, `MAX_ROOMS_PER_CLIENT`, "
            "`MAX_WS_CONNECTIONS`, `MAX_MESSAGE_BYTES`, `MAX_MSGS_PER_SEC`. Trocar por "
            "`mpsc::channel(N)` com backpressure. Aplicar `WebSocketUpgrade::max_message_size`. "
            "Adicionar heartbeat ping/pong com timeout de inatividade. Rejeitar `CreateRoom` "
            "acima do teto por IP (`fly-client-ip`)."
        ),
        accept=[
            "Teste de integração: 10 000 `CreateRoom` de um cliente resultam em <= MAX_ROOMS_PER_CLIENT salas e erro explícito depois.",
            "Mensagem de texto acima de `MAX_MESSAGE_BYTES` fecha a conexão sem alocar o corpo inteiro.",
            "Conexão sem tráfego é encerrada após o timeout de inatividade (teste com `tokio::time`).",
            "Canal de saída é limitado; envio a receptor saturado aplica backpressure em vez de crescer.",
        ],
    ),
    dict(
        id="F03",
        sev="ALTA",
        cat="3 · IDOR / BOLA",
        title="Reenvio de CreateRoom/JoinRoom no mesmo socket vaza salas e membros permanentemente",
        loc=[
            "crates/signaling/src/ws.rs:68-134",
            "crates/signaling/src/registry.rs:380-409",
        ],
        code=(
            "ClientMessage::CreateRoom { .. } => {\n"
            "    let (code, snapshot) = registry.create_room(.., tx.clone());\n"
            "    room_code = Some(code);          // sobrescreve o anterior\n"
            "    peer_id  = Some(snapshot.peer_id);\n"
            "}\n"
            "// ... ao fechar o socket:\n"
            "if let (Some(room), Some(id)) = (room_code, peer_id) {\n"
            "    registry.leave_room(&room, &id);  // limpa SÓ a última sala/peer\n"
            "}"
        ),
        why=(
            "Não há verificação de que a conexão já está vinculada a uma sala. Um segundo "
            "`CreateRoom` apenas sobrescreve as variáveis locais `room_code`/`peer_id`; a "
            "sala anterior mantém um `Member` com o `tx` deste socket, nunca fica vazia e "
            "`schedule_empty_room_cleanup` nunca roda — a sala vaza para sempre. Variante: "
            "`JoinRoom` na mesma sala N vezes com `device_id` distintos deixa N-1 membros "
            "órfãos que nunca são removidos na desconexão → a sala fica permanentemente "
            "'cheia' e indeletável (negação direcionada a uma sala)."
        ),
        impact=(
            "Vazamento de memória ilimitado e não autenticado (OOM); negação de serviço "
            "direcionada a uma sala específica (trava em 10/10 para sempre)."
        ),
        fix=(
            "Rejeitar `CreateRoom`/`JoinRoom` quando a conexão já tem `room_code`/`peer_id` "
            "(erro explícito), ou desfazer a associação anterior (`leave_room`) antes de "
            "criar/entrar na nova. Garantir 1 `peer_id` por conexão."
        ),
        accept=[
            "Teste: 2º `CreateRoom` na mesma conexão retorna erro e NÃO cria uma segunda sala.",
            "Teste: N `JoinRoom` na mesma sala pelo mesmo socket => 1 membro; ao desconectar a sala esvazia e é agendada para remoção.",
            "Contagem de salas volta a zero após todos os sockets fecharem (teste com `test-util` e avanço de tempo além do grace period).",
        ],
    ),
    dict(
        id="F04",
        sev="ALTA",
        cat="9 · Aplicação desktop (Electron)",
        title="verifyUpdateCodeSignature: false desativa a verificação de assinatura no auto-update",
        loc=["desktop/package.json:80-87", "desktop/src/main/updates.ts:32-44"],
        code=(
            '"win": {\n'
            '  "target": ["nsis", "portable"],\n'
            '  "icon": "icons/app-icon.png",\n'
            '  "verifyUpdateCodeSignature": false\n'
            "}\n"
            "// updates.ts: autoUpdater.checkForUpdatesAndNotify() — instala ao sair"
        ),
        why=(
            "Com `verifyUpdateCodeSignature: false`, o `electron-updater` (NSIS) não confere "
            "que o novo instalador foi assinado pelo mesmo publicador. A confiança no "
            "update passa a ser apenas 'veio via HTTPS do nosso GitHub Releases'. "
            "Comprometimento do token/conta do GitHub, de um asset de release ou do "
            "pipeline resulta em execução de código persistente e silenciosa em todos os "
            "clientes Windows ('instalar ao sair'). `asar: false` (linha 52) ainda "
            "distribui o app sem integridade de bundle."
        ),
        impact="RCE persistente e silenciosa em máquinas de usuários finais (cadeia de suprimento do desktop).",
        fix=(
            "Assinar os builds Windows e remover `verifyUpdateCodeSignature: false` (ou "
            "defini-lo como `true` explicitamente). Habilitar `asar: true`. Considerar "
            "`electron-updater` com chave pública própria e verificação de hash/assinatura "
            "do `latest.yml`."
        ),
        accept=[
            "Build de release Windows é assinado; `verifyUpdateCodeSignature` não aparece como `false`.",
            "Um `latest.yml`/instalador adulterado é rejeitado pelo updater (teste manual documentado).",
            "`asar` habilitado; `files` do electron-builder revisado.",
        ],
    ),
    dict(
        id="F05",
        sev="ALTA",
        cat="7 · Sinalização WebSocket / WebRTC",
        title="Handshake /ws aceita qualquer Origin (sem allowlist)",
        loc=["crates/signaling/src/ws.rs:26-34"],
        code=(
            "pub async fn ws_handler(\n"
            "    State(registry): State<Registry>,\n"
            "    State(turn): State<Option<TurnConfig>>,\n"
            "    headers: HeaderMap,\n"
            "    ws: WebSocketUpgrade,\n"
            ") -> impl IntoResponse {\n"
            "    let client_key = client_key(&headers);\n"
            "    ws.on_upgrade(move |socket| handle_socket(socket, registry, turn, client_key))\n"
            "    // nenhuma checagem de header Origin\n"
            "}"
        ),
        why=(
            "Qualquer página web (`evil.com`) pode abrir `wss://.../ws` e falar todo o "
            "protocolo de sinalização. Não há cookies de sessão, então não é um "
            "Cross-Site WebSocket Hijacking clássico de roubo de sessão; mas qualquer "
            "origem passa a poder criar salas, entrar em salas públicas, cunhar credenciais "
            "TURN e conduzir os abusos de F02/F03/F01. O OWASP WebSocket Cheat Sheet "
            "recomenda explicitamente uma allowlist de `Origin` no handshake."
        ),
        impact=(
            "Amplia a superfície de F01/F02/F03 para qualquer site que a vítima visite; "
            "abuso de recursos e de TURN a partir de origens arbitrárias."
        ),
        fix=(
            "Validar `Origin` contra uma allowlist configurável (a própria origem do app + "
            "`SCREEN_SHARE_URL` do desktop, se aplicável) e recusar o upgrade (403) caso "
            "não bata. Manter isso como defesa em profundidade, não como autenticação."
        ),
        accept=[
            "`Origin: https://evil.com` no handshake => resposta 403, sem `101 Switching Protocols`.",
            "`Origin` da própria app => upgrade normal; teste e2e continua verde.",
            "Allowlist vem de configuração/env, com um `const` documentando o default.",
        ],
    ),
    dict(
        id="F06",
        sev="MÉDIA",
        cat="3 · IDOR / BOLA",
        title="GET /api/rooms/:code sem autenticação nem rate limit expõe nome/ocupação e serve de oráculo de enumeração",
        loc=[
            "crates/signaling/src/rooms_status.rs:7-24",
            "apps/web/src/main.rs:41",
            "crates/signaling/src/registry.rs:273-280",
            "crates/signaling/src/registry.rs:488-498",
        ],
        code=(
            "pub async fn room_status_handler(\n"
            "    State(registry): State<Registry>,\n"
            "    Path(code): Path<String>,\n"
            ") -> Json<RoomStatus> {\n"
            "    match registry.room_status(&code) {\n"
            "        Some(summary) => Json(RoomStatus { exists: true,\n"
            "            name: Some(summary.name), member_count: Some(summary.member_count),\n"
            "            requires_password: Some(summary.requires_password) }),\n"
            "        None => Json(RoomStatus { exists: false, .. }),\n"
            "    }\n"
            "}\n"
            "// ROOM_CODE_ALPHABET = 31 símbolos, ROOM_CODE_LENGTH = 8  => ~2^39,6"
        ),
        why=(
            "Referência direta ao objeto (o código) sem qualquer verificação de posse ou "
            "de membresia. Para qualquer código existente retorna o NOME escolhido por "
            "humano (potencialmente sensível: 'Reunião Diretoria — Fusão X'), a ocupação e "
            "se há senha. Sem rate limit, é um oráculo de existência que torna a "
            "enumeração observável; para uma sala pública, o código é a única credencial, "
            "então basta adivinhá-lo para depois entrar."
        ),
        impact=(
            "Divulgação de informação (nome/atividade de salas) a qualquer um; "
            "reconhecimento e enumeração de salas; combinado com sala pública, acesso à "
            "sala."
        ),
        fix=(
            "Não devolver `name`/`member_count` para chamadas não autenticadas — devolver "
            "apenas `{ exists, requires_password }` (o mínimo para o fluxo de 'link "
            "morto'). Aplicar rate limit por IP neste endpoint. Opcional: código com mais "
            "entropia (>= 128 bits) para salas públicas, ou separar `room_id` de um "
            "`join_secret`."
        ),
        accept=[
            "Resposta do endpoint não contém o nome da sala nem a contagem de membros.",
            "Rate limit por IP aplicado (teste: N+1 chamadas em T segundos => 429).",
            "Fluxo de 'sala não encontrada' na UI continua funcionando.",
        ],
    ),
    dict(
        id="F07",
        sev="MÉDIA",
        cat="7 · Sinalização WebSocket / WebRTC",
        title="Sinalização Offer/Answer/IceCandidate/SetQuality aceita para qualquer co-membro sem relação de watch; cliente responde a Offer incondicionalmente",
        loc=[
            "crates/signaling/src/ws.rs:163-219",
            "apps/web/src/session/handler.rs:258-351",
        ],
        code=(
            "ClientMessage::Offer { to, sdp } => {\n"
            "    if let (Some(room), Some(from)) = (&room_code, &peer_id) {\n"
            "        registry.relay(room, &to, ServerMessage::Offer { from: from.clone(), sdp });\n"
            "    }\n"
            "}\n"
            "// handler.rs: ServerMessage::Offer { from, sdp } => cria RTCPeerConnection,\n"
            "//   seta ontrack -> <video id=\"video-{from}\">, create_answer, envia Answer,\n"
            "//   troca ICE — sem verificar se existe relação de watch com `from`."
        ),
        why=(
            "O servidor isola por sala (só entrega a membros da MESMA sala e `from` é "
            "definido pelo servidor — bom), mas não exige que os dois lados tenham "
            "concordado em se conectar. Um membro malicioso pode enviar `Offer { to: "
            "vítima }`; o cliente da vítima cria uma `RTCPeerConnection`, troca ICE "
            "(revelando candidatos host/srflx — IP de LAN e público — ao atacante sem "
            "consentimento) e responde. Também permite spam de renegociação, injeção de "
            "candidatos e `QualityRequested` para degradar o stream de outro espectador."
        ),
        impact=(
            "Divulgação de endereço IP (LAN + público) entre co-membros sem consentimento; "
            "consumo forçado de recursos; perturbação de conexões P2P dentro da sala."
        ),
        fix=(
            "No servidor: só relayar `Offer`/`Answer`/`IceCandidate`/`SetQuality` quando "
            "existir uma relação (sharer, viewer) registrada em `watchers` entre `from` e "
            "`to`. No cliente: ignorar `Offer` de um `from` que o usuário não escolheu "
            "assistir e que não é um espectador registrado."
        ),
        accept=[
            "Teste: `Offer` de A para B sem B ter pedido `WatchShare` de A (nem A de B) é descartado pelo servidor.",
            "Teste de cliente: `ServerMessage::Offer` de peer não relacionado não cria `RTCPeerConnection`.",
            "Cenário de watch legítimo (2 abas) continua estabelecendo mídia.",
        ],
    ),
    dict(
        id="F08",
        sev="MÉDIA",
        cat="5 · Entrada não tratada (XSS/spoofing)",
        title="nick / room_name / color sem limite de tamanho nem sanitização de caracteres de controle",
        loc=[
            "crates/protocol/src/client.rs:9-23",
            "apps/web/src/features/home/create_room.rs:43-59",
            "crates/signaling/src/registry.rs:109-160",
            "apps/web/src/features/room/member_card.rs:249",
            "apps/web/src/features/room/member_card.rs:278",
        ],
        code=(
            "CreateRoom { nick: String, password: Option<String>, room_name: String,\n"
            "             color: String, device_id: String }\n"
            "// create_room.rs: só `.trim()` + não-vazio.\n"
            "// registry.rs: nick/color/room_name armazenados verbatim e enviados a todos\n"
            "//   via PeerJoined / snapshot Joined.\n"
            "// member_card.rs: {m.nick} é renderizado (Leptos escapa HTML — sem XSS),\n"
            "//   mas sem limite de tamanho e sem filtrar \\n, RTL override, homoglifos."
        ),
        why=(
            "Não é XSS: o `view!` do Leptos escapa texto por padrão (ver Pontos Fortes). "
            "Mas: (a) um `nick` de vários MB é guardado em memória e RETRANSMITIDO a cada "
            "membro em `PeerJoined` e repetido no snapshot de entrada (amplificação) e "
            "injetado no DOM de todos; (b) `room_name` volta sem autenticação em "
            "`/api/rooms/:code` (F06); (c) caracteres de controle / override RTL / "
            "homoglifos em `nick` renderizam direto na UI dos outros membros, permitindo "
            "passar-se por outro membro ou pela etiqueta 'você'."
        ),
        impact="Amplificação de broadcast (DoS); falsificação de identidade visual na sala.",
        fix=(
            "Validar no servidor (fonte da verdade) e espelhar no cliente: `MAX_NICK_LEN` "
            "(~32), `MAX_ROOM_NAME_LEN` (~64), `color` restrito à allowlist da paleta "
            "(rejeitar, não só cair no default). Remover/rejeitar caracteres de controle e "
            "de formatação bidirecional. Normalizar Unicode (NFC)."
        ),
        accept=[
            "`CreateRoom`/`JoinRoom` com `nick` acima do limite é rejeitado com erro dedicado.",
            "`color` fora da paleta é rejeitado no servidor.",
            "`nick` com `\\n`/`\\u202E` é recusado ou saneado; teste unitário cobre.",
        ],
    ),
    dict(
        id="F09",
        sev="MÉDIA",
        cat="6 · DoS / exaustão de recursos",
        title="Limitador de brute force da senha confia em fly-client-ip sem validar proxy confiável",
        loc=[
            "crates/signaling/src/ws.rs:12-24",
            "crates/signaling/src/registry.rs:20-27",
            "crates/signaling/src/registry.rs:429-445",
        ],
        code=(
            "fn client_key(headers: &HeaderMap) -> String {\n"
            "    headers.get(\"fly-client-ip\")\n"
            "        .and_then(|v| v.to_str().ok())\n"
            "        .map(str::to_owned)\n"
            "        .unwrap_or_else(|| \"unknown\".to_string())\n"
            "}"
        ),
        why=(
            "O valor do header é usado como chave do balde de tentativas de senha "
            "(`MAX_PASSWORD_ATTEMPTS = 5 / 60 s`). Na Fly o header é reescrito pela borda e "
            "é confiável; mas não há verificação de que a requisição veio por um proxy "
            "confiável. Em qualquer implantação fora da Fly (ou se o app ficar acessível "
            "diretamente), o atacante rotaciona `fly-client-ip` a cada conexão e ignora "
            "totalmente o limite → brute force ilimitado da senha da sala. Quando o header "
            "está ausente, todos caem no balde único `\"unknown\"`, então 5 falhas de um "
            "atacante trancam todos os usuários daquela sala (DoS)."
        ),
        impact=(
            "Brute force ilimitado de senha de sala (fora da Fly / acesso direto); "
            "ou negação de entrada para usuários legítimos (balde global)."
        ),
        fix=(
            "Só confiar em `fly-client-ip`/`X-Forwarded-For` quando a conexão vier de uma "
            "lista de proxies confiáveis (env `TRUSTED_PROXIES`); caso contrário usar o IP "
            "real da conexão TCP (`ConnectInfo<SocketAddr>`). Nunca cair em uma chave "
            "constante compartilhada — usar o IP de peer real como fallback."
        ),
        accept=[
            "Teste: variar `fly-client-ip` a cada request sem vir de proxy confiável NÃO aumenta o número de tentativas permitidas.",
            "Sem header e sem proxy confiável, a chave é o IP de peer real, não `\"unknown\"`.",
            "Config de proxies confiáveis documentada.",
        ],
    ),
    dict(
        id="F10",
        sev="MÉDIA",
        cat="9 · Aplicação desktop (Electron)",
        title="Renderer privilegiado carrega origem remota sem bloqueio de navegação/janelas",
        loc=[
            "desktop/src/main/window.ts:16-36",
            "desktop/src/features/screen-share/picker.ts:35-48",
        ],
        code=(
            "mainWindow = new BrowserWindow({\n"
            "  width: 1100, height: 750, show: false,\n"
            "  webPreferences: { preload: path.join(__dirname, '..', 'preload.js') },\n"
            "});\n"
            "mainWindow.loadURL(APP_URL);  // https://screen-share-h0rb5w.fly.dev/\n"
            "// sem setWindowOpenHandler, sem will-navigate / will-redirect,\n"
            "// sem contextIsolation/sandbox/webSecurity explícitos"
        ),
        why=(
            "A janela principal carrega uma origem REMOTA e não há "
            "`webContents.setWindowOpenHandler`, nem handler de `will-navigate`/"
            "`will-redirect`. Um XSS ou open-redirect nessa origem (ou sequestro dela) "
            "pode navegar o renderer privilegiado para conteúdo do atacante, que então "
            "tem acesso às pontes IPC expostas (`desktopShare`, `picker`, `desktopAudio` "
            "— ver F11). Os defaults do Electron 43 (`contextIsolation`/`sandbox` ligados) "
            "evitam acesso direto ao Node, mas não as pontes. As flags de segurança não "
            "são fixadas explicitamente (fragilidade)."
        ),
        impact=(
            "Escalada de um XSS/redirect na web para o contexto do app desktop, com acesso "
            "às pontes IPC (ver impacto em F11)."
        ),
        fix=(
            "`webContents.setWindowOpenHandler(() => ({ action: 'deny' }))`; bloquear "
            "`will-navigate`/`will-redirect` para fora da origem do app; fixar "
            "`contextIsolation: true`, `sandbox: true`, `nodeIntegration: false`, "
            "`webSecurity: true` explicitamente; considerar `session` com CSP injetada."
        ),
        accept=[
            "`window.open(...)` e navegação para `https://evil.com` a partir do renderer são bloqueadas (teste `_electron`).",
            "webPreferences fixa as 4 flags explicitamente.",
            "e2e desktop continua verde.",
        ],
    ),
    dict(
        id="F11",
        sev="MÉDIA",
        cat="9 · Aplicação desktop (Electron)",
        title="Pontes IPC potentes expostas à página remota sem checagem de origem/emissor",
        loc=[
            "desktop/src/preload.ts:9-49",
            "desktop/src/features/audio-share/ipc.ts:12-26",
            "desktop/src/platform/windows/audio.ts:19-32",
            "desktop/src/features/screen-share/quick-share.ts:11-32",
        ],
        code=(
            "contextBridge.exposeInMainWorld('desktopAudio', {\n"
            "  start: (target) => ipcRenderer.invoke('start-audio-loopback', target),\n"
            "  stop:  () => ipcRenderer.invoke('stop-audio-loopback'),\n"
            "});\n"
            "contextBridge.exposeInMainWorld('picker', { listAudioApps: () => ipcRenderer.invoke('list-audio-apps'), .. });\n"
            "contextBridge.exposeInMainWorld('desktopShare', { linkReady: (l) => ipcRenderer.send('desktop-share:link-ready', l), .. });\n"
            "// ipc.ts: ipcMain.handle('start-audio-loopback', (_e, target) => startAudioLoopback(target)) — sem checar _e.senderFrame"
        ),
        why=(
            "O preload é compartilhado com a janela principal, que carrega a origem "
            "remota. Nenhum `ipcMain.on/handle` verifica `event.senderFrame`/origem. "
            "A partir de um XSS na web (ou via F10) a página pode: "
            "`picker.listAudioApps()` / `desktopAudio.start({mode:'screen',excludedBinaries:[]})` "
            "→ enumerar apps em execução e nomes/caminhos de executáveis, e INICIAR uma "
            "captura real de áudio do sistema sem consentimento nem seletor; no Windows o "
            "PCM misturado é então empurrado para o renderer via "
            "`desktop-audio-pcm-chunk` (`platform/windows/audio.ts:26-29`) — captura "
            "encoberta do áudio do sistema exfiltrável pela página. "
            "`desktopShare.linkReady(\"qualquer coisa\")` sobrescreve a área de "
            "transferência; `memberJoined` gera notificação de SO com texto arbitrário."
        ),
        impact=(
            "Captura encoberta de áudio do sistema (Windows); enumeração de processos do "
            "usuário; sequestro da área de transferência; spoofing de notificação."
        ),
        fix=(
            "Verificar `event.senderFrame` (origem == origem do app) em todo handler "
            "`ipcMain`. Não empurrar PCM para o renderer sem um sinal de sessão ativa "
            "iniciada pelo próprio app. Restringir `linkReady` a um formato de link "
            "esperado. Idealmente expor as pontes só quando o conteúdo local/confiável "
            "estiver carregado."
        ),
        accept=[
            "Todo handler IPC rejeita mensagens cujo `senderFrame` não seja a origem do app (teste unitário).",
            "`desktopAudio.start` sem uma sessão de compartilhamento ativa não emite `desktop-audio-pcm-chunk`.",
            "`linkReady` valida o formato antes de escrever no clipboard.",
        ],
    ),
    dict(
        id="F12",
        sev="BAIXA",
        cat="10 · Hardening HTTP",
        title="Sem Content-Security-Policy / HSTS / X-Content-Type-Options / Permissions-Policy",
        loc=["apps/web/src/main.rs:39-56", "apps/web/src/app.rs:14-39"],
        code=(
            "let app = Router::new()\n"
            "    .leptos_routes(&leptos_options, routes, move || shell(..))\n"
            "    .fallback(leptos_axum::file_and_error_handler(shell))\n"
            "    .with_state(leptos_options)\n"
            "    .merge(signaling_router);\n"
            "// nenhum middleware tower-http de cabeçalhos de segurança"
        ),
        why=(
            "Nenhum middleware adiciona cabeçalhos de segurança. Sem CSP, um XSS "
            "hipotético (ou a superfície de F10/F11) roda sem restrição; sem "
            "`Strict-Transport-Security` o `force_https` da Fly só redireciona; sem "
            "`Permissions-Policy` não há restrição declarada de `display-capture`/"
            "`camera`/`microphone`; sem `X-Content-Type-Options: nosniff` sobra sniffing "
            "de MIME nos assets."
        ),
        impact="Defesa em profundidade ausente; menor contenção de XSS e de abuso das pontes desktop.",
        fix=(
            "Adicionar `tower_http::set_header`/`SetResponseHeaderLayer` (ou "
            "`tower-helmet`) com CSP restritiva (`default-src 'self'`; `connect-src 'self' "
            "wss://... stun:/turn:`; `img-src 'self' data:`; `style-src 'self' "
            "fonts.googleapis.com`; `font-src fonts.gstatic.com`), `HSTS`, "
            "`X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, "
            "`Permissions-Policy` restrito."
        ),
        accept=[
            "Resposta HTTP inclui CSP, HSTS, `X-Content-Type-Options`, `Referrer-Policy`, `Permissions-Policy`.",
            "App e fontes do Google carregam sem violação de CSP no console.",
            "Teste de fumaça (criar/entrar/assistir) continua passando.",
        ],
    ),
    dict(
        id="F13",
        sev="BAIXA",
        cat="4 · Chaves expostas (segredos)",
        title="Realm com default público e ausência de validação de startup rejeitando TURN_SECRET fraco/placeholder",
        loc=["docker-entrypoint.sh:11-37", "crates/signaling/src/turn.rs:31-50"],
        code=(
            'if [ -n "$TURN_SECRET" ] && [ -n "$TURN_EXTERNAL_IP" ]; then\n'
            '  turnserver ... --realm="${TURN_REALM:-screenshare}" \\\n'
            '             --static-auth-secret="$TURN_SECRET" ...\n'
            "# from_vars: só rejeita string vazia; qualquer valor não-vazio serve"
        ),
        why=(
            "`--realm` cai num default público (`screenshare`) sem asserção de startup. "
            "Mais relevante: `TurnConfig::from_vars` só recusa `TURN_SECRET` vazio — um "
            "valor placeholder fraco (ex.: `changeme`) é aceito silenciosamente por "
            "coturn e por `mint_credentials`, sem nenhum gate que rejeite defaults "
            "conhecidos."
        ),
        impact="Segredo TURN fraco não detectado no boot; realm previsível.",
        fix=(
            "Validar no startup: `TURN_SECRET` com comprimento/entropia mínimos e não "
            "pertencente a uma denylist de placeholders; abortar o processo se falhar. "
            "Tornar `TURN_REALM` obrigatório quando TURN está ligado (sem default)."
        ),
        accept=[
            "Processo recusa iniciar com `TURN_SECRET` curto ou em denylist (teste).",
            "Sem `TURN_REALM` explícito com TURN ligado => erro de configuração no boot.",
        ],
    ),
    dict(
        id="F14",
        sev="BAIXA",
        cat="4 · Chaves expostas (segredos)",
        title="Senha da sala guardada em texto puro em sessionStorage",
        loc=["apps/web/src/infra/storage.rs:32-64"],
        code=(
            "pub struct RoomSession {\n"
            "    pub nick: String,\n"
            "    pub color: String,\n"
            "    pub password: Option<String>,  // texto puro, chave screen_share_room_session_<code>\n"
            "}"
        ),
        why=(
            "A senha da sala é persistida em `sessionStorage` (escopo de aba, apagada ao "
            "fechar) para permitir o rejoin silencioso após reload. É uma escolha "
            "deliberada, mas qualquer script na origem (um XSS, ou a superfície das pontes "
            "desktop) consegue lê-la e exfiltrá-la junto com o nick."
        ),
        impact="Exposição da senha da sala a script na origem (defesa em profundidade).",
        fix=(
            "Preferir não persistir a senha: manter um token de rejoin de curta duração "
            "emitido pelo servidor em vez da senha. Se mantida, documentar o risco e "
            "reforçar CSP (F12)."
        ),
        accept=[
            "Rejoin após reload funciona sem gravar a senha em claro (token de sessão), OU risco aceito e documentado em ADR com CSP aplicada.",
        ],
    ),
    dict(
        id="F15",
        sev="BAIXA",
        cat="7 · Sinalização WebSocket / WebRTC",
        title="add_watcher aceita sharer_id arbitrário; ReportLatency aceita ping arbitrário",
        loc=[
            "crates/signaling/src/registry.rs:311-327",
            "crates/signaling/src/registry.rs:349-366",
        ],
        code=(
            "pub fn add_watcher(&self, room_code: &str, sharer_id: &str, viewer_id: &str) {\n"
            "    room.watchers.entry(sharer_id.to_string()).or_default().insert(viewer_id.to_string());\n"
            "    // sharer_id não é validado como membro nem como sharer ativo\n"
            "}\n"
            "// report_latency: `ms: u32` do cliente é retransmitido como o ping daquele peer"
        ),
        why=(
            "`add_watcher` cria entradas em `watchers` para `sharer_id` que não é membro "
            "nem está compartilhando (poluição de estado, `WatchersChanged` espúrio). "
            "`ReportLatency` aceita qualquer `ms` e o servidor retransmite como o ping "
            "medido daquele peer (spoofing cosmético do indicador de ping alheio)."
        ),
        impact="Poluição de estado do mapa de watchers; falsificação do indicador de ping de outros membros.",
        fix=(
            "`add_watcher`: ignorar se `sharer_id` não estiver em `room.members` (ou não "
            "em `room.sharers`). `report_latency`: só aceitar para o próprio `peer_id` da "
            "conexão (já é o caso) e limitar `ms` a um teto plausível."
        ),
        accept=[
            "`WatchShare` para um `sharer_id` inexistente não cria entrada em `watchers`.",
            "`ReportLatency` acima do teto é descartado.",
        ],
    ),
    dict(
        id="F16",
        sev="BAIXA",
        cat="8 · TURN / infraestrutura",
        title="--no-tls --no-dtls: canal de controle STUN/TURN em texto puro",
        loc=["docker-entrypoint.sh:35-37", "fly.toml:25"],
        code='turnserver ... --no-tls --no-dtls &\nTURN_URLS = "turn:137.66.9.162:3478"   # não "turns:"',
        why=(
            "O canal de controle STUN/TURN trafega sem TLS/DTLS. A mídia continua "
            "protegida por SRTP (a confidencialidade da tela não é perdida), mas some uma "
            "camada e o relay não atravessa firewalls restritivos que só liberam 443/TLS."
        ),
        impact="Metadados de sinalização TURN observáveis na rede; menor alcance de conectividade.",
        fix="Habilitar `turns:` (TLS) na 5349 com certificado; manter `turn:` como fallback.",
        accept=[
            "coturn escuta `turns:` com certificado válido; `TURN_URLS` inclui a URL `turns:`.",
            "Teste de sala real com TURN forçado (`iceTransportPolicy: relay`) passa via TLS.",
        ],
    ),
    dict(
        id="F17",
        sev="INFORMATIVA",
        cat="10 · Hardening HTTP",
        title="Ação de GitHub sem pin de versão (setup-flyctl@master)",
        loc=[".github/workflows/ci-cd.yml (job deploy-web)"],
        code="- uses: superfly/flyctl-actions/setup-flyctl@master",
        why=(
            "`@master` acompanha o branch: um comprometimento do repositório da ação "
            "executa código arbitrário no runner de deploy, que tem acesso a "
            "`secrets.FLY_API_TOKEN`."
        ),
        impact="Risco de cadeia de suprimento no pipeline de deploy.",
        fix="Fixar a ação por SHA de commit (ou tag imutável).",
        accept=["Todas as `uses:` de terceiros fixadas por SHA."],
    ),
    dict(
        id="F18",
        sev="INFORMATIVA",
        cat="9 · Aplicação desktop (Electron)",
        title="asar: false — app empacotado sem integridade de bundle",
        loc=["desktop/package.json:52"],
        code='"asar": false',
        why=(
            "Sem `asar` (e sem `asintegrity`), os arquivos do app ficam soltos no disco, "
            "facilitando adulteração local persistente do código do app."
        ),
        impact="Menor resistência a adulteração local; sem verificação de integridade do bundle.",
        fix="`asar: true` e, no Electron recente, habilitar `asarIntegrity`.",
        accept=["Build empacotado usa `asar`; verificação de integridade habilitada."],
    ),
    dict(
        id="F19",
        sev="INFORMATIVA",
        cat="4 · Chaves expostas (segredos)",
        title="E-mail pessoal do mantenedor versionado em desktop/package.json",
        loc=["desktop/package.json:5-8"],
        code='"author": { "name": "hiroshimorowaka", "email": "guilhermecabral1204@gmail.com" }',
        why="Não é segredo, mas é PII exposta e reutilizável para phishing/enumeração.",
        impact="Exposição de PII.",
        fix="Usar um e-mail de projeto/alias em vez de e-mail pessoal.",
        accept=["`author.email` aponta para um endereço de projeto."],
    ),
]

STRENGTHS = [
    dict(
        title="Sem sink de XSS no frontend; escape automático do Leptos",
        ev=(
            "grep por `inner_html` / `set_inner_html` / `dangerously*` / `eval(` / "
            "`new Function` em `apps/web` e `desktop` retorna vazio. O `view!` do Leptos "
            "escapa texto por padrão, então `nick`/`room_name`/nomes de watchers/status — "
            "todos controlados pelo cliente — renderizam com segurança "
            "(`apps/web/src/features/room/member_card.rs:249,278`)."
        ),
    ),
    dict(
        title="color nunca renderizado cru — allowlist de paleta",
        ev=(
            "`components/palette.rs:21-27` mapeia `color` por uma tabela fixa de 10 "
            "entradas e cai num `#b0b8c1` seguro para valor desconhecido; o binding "
            "`style=\"--member: {…}\"` em `member_card.rs:220` não dá para injeção de CSS."
        ),
    ),
    dict(
        title="peer_id gerado no servidor; from das mensagens é do servidor",
        ev=(
            "`registry.rs:119,209` usam `Uuid::new_v4()`. Em `ws.rs`, `Offer`/`Answer`/"
            "`IceCandidate` são retransmitidos com `from: from.clone()` (o `peer_id` da "
            "conexão), nunca um valor do cliente — impersonation por ID não é possível."
        ),
    ),
    dict(
        title="Isolamento entre salas íntegro",
        ev=(
            "Toda operação do registry resolve `rooms.get(room_code)` com o `room_code` "
            "vinculado à conexão no join (`registry.rs:relay/add_watcher/start_share/...`). "
            "Não há IDOR entre salas — um membro não consegue endereçar outra sala."
        ),
    ),
    dict(
        title="Senha de sala: argon2id, verificada no servidor, sem bypass por vazio",
        ev=(
            "`auth.rs` usa o crate `argon2` (não implementação própria); "
            "`registry.rs:check_optional_password` exige senha não-vazia e "
            "`verify_password` correto; senha em branco só entra em sala explicitamente "
            "pública."
        ),
    ),
    dict(
        title="Proteção contra brute force de senha existe (janela deslizante, por cliente)",
        ev=(
            "`registry.rs:20-27,429-445`: `MAX_PASSWORD_ATTEMPTS = 5` em "
            "`PASSWORD_ATTEMPT_WINDOW = 60 s`, escopo por cliente (não por sala, para não "
            "trancar terceiros); `TooManyAttempts` é devolvido mesmo se a senha desta vez "
            "estiver correta (ver F09 para a ressalva do proxy)."
        ),
    ),
    dict(
        title="Limite de membros aplicado no servidor",
        ev="`registry.rs:205` recusa join quando `room.members.len() >= MAX_MEMBERS` (10) — não confia na UI.",
    ),
    dict(
        title="Sem segredos no código, no histórico git ou no bundle do frontend",
        ev=(
            "`git log -p --all` varrido — nenhum segredo commitado. `TURN_SECRET` é um "
            "Fly secret (`fly.toml:12` documenta `fly secrets set`), não está em "
            "`fly.toml`. As credenciais TURN entregues ao browser são HMAC de curta "
            "duração cunhadas no servidor e só após autenticação (`turn.rs:58-75`, "
            "`media.rs`)."
        ),
    ),
    dict(
        title="coturn usa use-auth-secret (sem alocações anônimas)",
        ev="`docker-entrypoint.sh:27-28` — `--use-auth-secret` + `--static-auth-secret`. (Hardening de peer-IP/cotas ainda falta: F01.)",
    ),
    dict(
        title="CI com privilégio mínimo e sem exposição de segredo a PR de fork",
        ev=(
            "`.github/workflows/ci-cd.yml:37` `permissions: contents: read`; gatilho "
            "`pull_request` (não `pull_request_target`); `secrets.FLY_API_TOKEN` só no job "
            "`deploy-web`, condicionado a `github.event_name != 'pull_request'`."
        ),
    ),
    dict(
        title="Sem injeção de comando no desktop",
        ev=(
            "Todos os `spawn`/`runCollectingStdout` usam array de argv, sem `shell: true` "
            "(`platform/run-command.ts`, `platform/linux/pipewire.ts`). As strings "
            "`binary`/`excludedBinaries` vindas do renderer são apenas comparadas "
            "(`===`, `.includes`), nunca passadas a um shell."
        ),
    ),
    dict(
        title="Sem panic acionável por entrada hostil no servidor de sinalização",
        ev=(
            "Cada `expect()` revisado: mutex do registry (`registry.rs:106`) nunca é "
            "mantido através de panic; relógio (`turn.rs:61`); HMAC aceita chave de "
            "qualquer tamanho (`turn.rs:66`); `ServerMessage` sempre serializa "
            "(`ws.rs:48`); argon2 sobre `&str` UTF-8 válido (`auth.rs:9`). JSON malformado "
            "no WS é silenciosamente ignorado (`ws.rs:64`)."
        ),
    ),
    dict(
        title="Seletor de captura explícito no desktop",
        ev=(
            "`display-media.ts` implementa `setDisplayMediaRequestHandler` com uma janela "
            "de seleção própria — nunca captura tela cheia silenciosamente. Electron 43 "
            "(`contextIsolation`/`sandbox` ligados por padrão)."
        ),
    ),
    dict(
        title="Código de sala de fonte CSPRNG, não sequencial",
        ev=(
            "`registry.rs:488-498`: 8 caracteres sobre um alfabeto de 31 símbolos sem "
            "ambiguidade, de `rand::rng()` (ChaCha) — ~2^39,6, não incremental, não "
            "previsível. (Ainda assim, ver F06 sobre o oráculo de enumeração.)"
        ),
    ),
]

RECOMMENDATIONS = [
    dict(
        p="P1",
        text=(
            "Endurecer o coturn (F01): `--denied-peer-ip` para loopback/link-local/RFC1918/"
            "6PN, `--total-quota`/`--user-quota`/`--max-bps`/`--bps-capacity`, "
            "`--no-multicast-peers`; reduzir o TTL da credencial. Bloqueia SSRF a "
            "metadados de nuvem e abuso de banda."
        ),
    ),
    dict(
        p="P1",
        text=(
            "Impor limites de recurso no servidor de sinalização (F02, F03): "
            "`MAX_ROOMS` global e por IP, teto de conexões, `max_message_size`, rate limit "
            "de mensagens, heartbeat + idle timeout, canal mpsc limitado; rejeitar "
            "`CreateRoom`/`JoinRoom` em conexão já vinculada."
        ),
    ),
    dict(
        p="P1",
        text=(
            "Restaurar a integridade do auto-update do desktop (F04): assinar os builds "
            "Windows, remover `verifyUpdateCodeSignature: false`, habilitar `asar`."
        ),
    ),
    dict(
        p="P2",
        text=(
            "Allowlist de `Origin` no handshake `/ws` (F05) e validação de proxy confiável "
            "para o `client_key` do limitador de senha (F09)."
        ),
    ),
    dict(
        p="P2",
        text=(
            "Autorização de sinalização par-a-par (F07): só relayar/aceitar `Offer`/"
            "`Answer`/`IceCandidate`/`SetQuality` quando existir relação (sharer, viewer) "
            "registrada; cliente ignora `Offer` não solicitada."
        ),
    ),
    dict(
        p="P2",
        text=(
            "Minimizar `GET /api/rooms/:code` (F06): não devolver nome/ocupação sem "
            "autenticação; rate limit por IP."
        ),
    ),
    dict(
        p="P2",
        text=(
            "Desktop: bloquear navegação/`window.open` do renderer e verificar "
            "`event.senderFrame` em todo handler IPC; não empurrar PCM sem sessão ativa "
            "(F10, F11)."
        ),
    ),
    dict(
        p="P3",
        text=(
            "Validação de entrada no protocolo (F08, F15): limites de tamanho para "
            "`nick`/`room_name`, `color` restrito à paleta, rejeição de caracteres de "
            "controle/bidi, teto para `ReportLatency`, `add_watcher` só para membros."
        ),
    ),
    dict(
        p="P3",
        text=(
            "Cabeçalhos de segurança / CSP no app Axum (F12); validação de startup do "
            "`TURN_SECRET` e do realm (F13); `turns:` TLS no coturn (F16)."
        ),
    ),
    dict(
        p="P3",
        text=(
            "Higiene: token de rejoin em vez de senha em `sessionStorage` (F14); pin de "
            "ações de CI por SHA (F17); e-mail de projeto no `package.json` (F19)."
        ),
    ),
]

# GitHub issues: (numero, titulo, labels, corpo_markdown)
GH_ISSUES = [
    (
        1,
        "[Segurança] coturn sem allowlist de peer-IP e sem cotas (SSRF a metadados + abuso de banda)",
        "security, severity:critical, infra",
        """## Problema
O `docker-entrypoint.sh` sobe o coturn com `--use-auth-secret` (bom), mas **sem** `--denied-peer-ip` e **sem** cotas de banda/alocação. Qualquer participante de qualquer sala recebe uma credencial TURN válida por 6 h no snapshot `Joined`; para uma sala pública, isso é qualquer pessoa que conheça o código.

## Por que é explorável
Com a credencial, um atacante pede alocações e faz o coturn encaminhar pacotes para endereços arbitrários: `169.254.169.254` (metadados de nuvem), RFC1918, `fdaa::/16` (6PN da Fly), etc. coturn recente nega loopback por padrão, mas **não** nega link-local nem RFC1918. Sem `--total-quota`/`--max-bps`, o relay também vira amplificador de tráfego.

## Evidência
`docker-entrypoint.sh:24-37`
```sh
turnserver --no-cli --fingerprint --use-auth-secret \\
  --static-auth-secret="$TURN_SECRET" --realm="${TURN_REALM:-screenshare}" \\
  --listening-port=3478 --min-port=... --max-port=... \\
  --external-ip="$TURN_EXTERNAL_IP" --log-file=stdout --no-tls --no-dtls &
# sem --denied-peer-ip / --allowed-peer-ip
# sem --total-quota / --user-quota / --max-bps / --bps-capacity
```
`crates/signaling/src/turn.rs:16` — `CREDENTIAL_TTL = 6 h`.

## Impacto
SSRF a partir da infraestrutura de relay (varredura interna + roubo potencial de tokens do endpoint de metadados); abuso de banda / DoS por exaustão de alocações.

## Sugestão de correção
- `--denied-peer-ip` para `0.0.0.0/8`, `10/8`, `100.64/10`, `169.254/16`, `172.16/12`, `192.168/16`, `::1`, `fc00::/7`, `fe80::/10`; liberar só o necessário com `--allowed-peer-ip`.
- `--total-quota`, `--user-quota`, `--max-bps`, `--bps-capacity`, `--no-multicast-peers`.
- Reduzir `CREDENTIAL_TTL` para ~1 h.

## Critérios de aceite
- [ ] `turnutils_uclient` contra `169.254.169.254` e um IP RFC1918 falham (peer negado).
- [ ] Alocações acima da cota são rejeitadas; `--max-bps` presente no processo em execução.
- [ ] Teste de sala real (2 abas, mídia P2P) continua passando.
- [ ] `CREDENTIAL_TTL` atualizado e comentado com a razão.
""",
    ),
    (
        2,
        "[Segurança] Servidor de sinalização sem limites de recurso (salas, conexões, tamanho/taxa de mensagem, canal ilimitado)",
        "security, severity:high, dos",
        """## Problema
Não há `MAX_ROOMS` global nem por IP, teto de conexões WebSocket, limite de tamanho/taxa de mensagem, `idle timeout` ou heartbeat. O canal de saída é `mpsc::unbounded_channel`.

## Por que é explorável
Um único socket (sem autenticação prévia) pode criar salas em massa, abrir milhares de conexões ociosas (slowloris) ou inundar sinalização. Uma `sdp` de dezenas de MB (limite default do axum-ws ~64 MiB) repassada a um receptor lento cresce sem teto na fila dele. A VM de produção tem 256 MB e `min_machines_running = 0`.

## Evidência
`crates/signaling/src/ws.rs:43,62-66`
```rust
let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
while let Some(Ok(msg)) = ws_receiver.next().await {
    let Message::Text(text) = msg else { continue };
    let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) else { continue };
    // sem limite de tamanho, sem rate limit, sem timeout
```
`crates/signaling/src/registry.rs:90-160` — `HashMap<String, Room>` sem teto.

## Impacto
Negação de serviço remota, não autenticada e trivial (OOM / esgotamento de descritores).

## Sugestão de correção
`const` nomeadas: `MAX_ROOMS`, `MAX_ROOMS_PER_CLIENT`, `MAX_WS_CONNECTIONS`, `MAX_MESSAGE_BYTES`, `MAX_MSGS_PER_SEC`. `mpsc::channel(N)` com backpressure. `WebSocketUpgrade::max_message_size`. Heartbeat ping/pong + idle timeout. Rate limit de `CreateRoom` por `fly-client-ip`.

## Critérios de aceite
- [ ] 10 000 `CreateRoom` de um cliente => <= `MAX_ROOMS_PER_CLIENT` e erro explícito depois.
- [ ] Texto acima de `MAX_MESSAGE_BYTES` fecha a conexão sem alocar o corpo inteiro.
- [ ] Conexão ociosa é encerrada após o timeout (teste com `tokio::time`).
- [ ] Canal de saída limitado; receptor saturado sofre backpressure.
""",
    ),
    (
        3,
        "[Segurança] Reenvio de CreateRoom/JoinRoom no mesmo socket vaza salas e membros permanentemente",
        "security, severity:high, dos",
        """## Problema
A conexão não verifica se já está vinculada a uma sala. Um segundo `CreateRoom` sobrescreve `room_code`/`peer_id` locais; a sala anterior mantém um `Member` com o `tx` do socket, nunca esvazia e nunca é removida.

## Por que é explorável
Loop de `CreateRoom` num socket => salas permanentes ilimitadas (OOM). Variante: `JoinRoom` na mesma sala N vezes com `device_id` distintos => N-1 membros órfãos nunca removidos => sala presa em 10/10 e indeletável (negação direcionada).

## Evidência
`crates/signaling/src/ws.rs:68-134`
```rust
ClientMessage::CreateRoom { .. } => {
    let (code, snapshot) = registry.create_room(.., tx.clone());
    room_code = Some(code);           // sobrescreve
    peer_id  = Some(snapshot.peer_id);
}
// ao fechar: registry.leave_room(&room, &id)  -> limpa SÓ a última
```
`crates/signaling/src/registry.rs:380-409` — cleanup só quando `members.is_empty()`.

## Impacto
Vazamento de memória não autenticado (OOM); DoS direcionado a uma sala.

## Sugestão de correção
Rejeitar `CreateRoom`/`JoinRoom` quando a conexão já tem `room_code`/`peer_id`, ou desfazer a associação anterior antes. Garantir 1 `peer_id` por conexão.

## Critérios de aceite
- [ ] 2º `CreateRoom` na mesma conexão retorna erro e não cria segunda sala.
- [ ] N `JoinRoom` na mesma sala pelo mesmo socket => 1 membro; desconectar esvazia a sala.
- [ ] Contagem de salas volta a zero após todos os sockets fecharem (teste com avanço de tempo).
""",
    ),
    (
        4,
        "[Segurança] verifyUpdateCodeSignature: false desativa a verificação de assinatura do auto-update",
        "security, severity:high, desktop, supply-chain",
        """## Problema
`desktop/package.json` define `verifyUpdateCodeSignature: false` para o alvo Windows. O `electron-updater` (NSIS) deixa de conferir que o novo instalador foi assinado pelo mesmo publicador.

## Por que é explorável
A confiança no update passa a ser só "HTTPS + está no nosso GitHub Releases". Comprometimento do token/conta do GitHub, de um asset de release ou do pipeline => RCE persistente e silenciosa em todos os clientes Windows ("instalar ao sair"). `asar: false` piora (sem integridade de bundle).

## Evidência
`desktop/package.json:80-87`
```json
"win": { "target": ["nsis", "portable"], "icon": "icons/app-icon.png",
         "verifyUpdateCodeSignature": false }
```
`desktop/src/main/updates.ts:32-44` — `autoUpdater.checkForUpdatesAndNotify()`.

## Impacto
RCE persistente em máquinas de usuários finais.

## Sugestão de correção
Assinar os builds Windows; remover `verifyUpdateCodeSignature: false`; habilitar `asar: true` + `asarIntegrity`.

## Critérios de aceite
- [ ] Build de release Windows assinado; flag não aparece como `false`.
- [ ] `latest.yml`/instalador adulterado é rejeitado pelo updater (teste manual documentado).
- [ ] `asar` habilitado.
""",
    ),
    (
        5,
        "[Segurança] Handshake /ws aceita qualquer Origin (sem allowlist)",
        "security, severity:high, websocket",
        """## Problema
`ws_handler` faz o upgrade sem checar o header `Origin`.

## Por que é explorável
Qualquer página web pode abrir `wss://.../ws` e falar todo o protocolo: criar salas, entrar em salas públicas, cunhar credenciais TURN e conduzir os abusos de DoS/TURN. Não há cookies (então não é CSWSH clássico de sessão), mas o OWASP WebSocket Cheat Sheet recomenda allowlist de `Origin` no handshake.

## Evidência
`crates/signaling/src/ws.rs:26-34`
```rust
pub async fn ws_handler(State(registry): State<Registry>, State(turn): State<Option<TurnConfig>>,
                        headers: HeaderMap, ws: WebSocketUpgrade) -> impl IntoResponse {
    let client_key = client_key(&headers);
    ws.on_upgrade(move |socket| handle_socket(socket, registry, turn, client_key))
    // nenhuma checagem de Origin
}
```

## Impacto
Amplia a superfície de abuso para qualquer site que a vítima visite.

## Sugestão de correção
Validar `Origin` contra allowlist configurável (origem do app + `SCREEN_SHARE_URL`), recusar upgrade (403) caso não bata. Defesa em profundidade, não autenticação.

## Critérios de aceite
- [ ] `Origin: https://evil.com` => 403, sem `101 Switching Protocols`.
- [ ] `Origin` da própria app => upgrade normal; e2e verde.
- [ ] Allowlist vem de env, com `const` documentando o default.
""",
    ),
    (
        6,
        "[Segurança] GET /api/rooms/:code sem auth nem rate limit expõe nome/ocupação e serve de oráculo de enumeração",
        "security, severity:medium, idor, info-disclosure",
        """## Problema
O endpoint devolve, para qualquer código existente e sem autenticação, o nome (escolhido por humano), a ocupação e o flag de senha. Sem rate limit.

## Por que é explorável
Referência direta ao objeto (código) sem verificação de posse/membresia. O nome pode ser sensível ("Reunião Diretoria — Fusão X"). Sem rate limit é um oráculo de existência que torna a enumeração observável; para sala pública, adivinhar o código = entrar.

## Evidência
`crates/signaling/src/rooms_status.rs:7-24`
```rust
Some(summary) => Json(RoomStatus { exists: true, name: Some(summary.name),
    member_count: Some(summary.member_count), requires_password: Some(summary.requires_password) }),
```
`crates/signaling/src/registry.rs:488-498` — código: alfabeto de 31, comprimento 8 (~2^39,6).

## Impacto
Divulgação de informação; reconhecimento/enumeração de salas; acesso a sala pública.

## Sugestão de correção
Devolver apenas `{ exists, requires_password }` sem autenticação. Rate limit por IP. Opcional: mais entropia no código de sala pública, ou separar `room_id` de `join_secret`.

## Critérios de aceite
- [ ] Resposta não contém nome nem contagem de membros.
- [ ] Rate limit por IP (teste: N+1 chamadas em T s => 429).
- [ ] Fluxo de "sala não encontrada" na UI continua funcionando.
""",
    ),
    (
        7,
        "[Segurança] Sinalização Offer/Answer/IceCandidate aceita para qualquer co-membro sem relação de watch",
        "security, severity:medium, webrtc, privacy",
        """## Problema
O servidor relaya `Offer`/`Answer`/`IceCandidate`/`SetQuality` para qualquer `to` que seja membro da sala, sem exigir uma relação (sharer, viewer). O cliente responde a `ServerMessage::Offer` incondicionalmente.

## Por que é explorável
Um membro malicioso envia `Offer { to: vítima }`; o cliente da vítima cria `RTCPeerConnection`, troca ICE (revelando IP de LAN e público ao atacante sem consentimento) e responde. Também permite spam de renegociação, injeção de candidatos e `QualityRequested` degradando o stream de terceiros.

## Evidência
`crates/signaling/src/ws.rs:163-207`
```rust
ClientMessage::Offer { to, sdp } => {
    if let (Some(room), Some(from)) = (&room_code, &peer_id) {
        registry.relay(room, &to, ServerMessage::Offer { from: from.clone(), sdp });
    }
}
```
`apps/web/src/session/handler.rs:258-351` — cria PC, `ontrack` -> `<video id="video-{from}">`, `create_answer`, troca ICE, sem checar relação com `from`.

## Impacto
Divulgação de IP entre co-membros sem consentimento; consumo forçado de recursos; perturbação de conexões P2P.

## Sugestão de correção
- Servidor: só relayar quando existir `(sharer, viewer)` em `room.watchers` entre `from` e `to`.
- Cliente: ignorar `Offer` de `from` que o usuário não escolheu assistir e que não é espectador registrado.

## Critérios de aceite
- [ ] `Offer` de A para B sem relação de watch é descartada pelo servidor (teste).
- [ ] Cliente não cria `RTCPeerConnection` para `Offer` de peer não relacionado.
- [ ] Cenário de watch legítimo (2 abas) continua estabelecendo mídia.
""",
    ),
    (
        8,
        "[Segurança] Campos de protocolo sem validação (nick/room_name/color/ReportLatency/WatchShare)",
        "security, severity:medium, input-validation",
        """## Problema
`nick`, `room_name` e `color` não têm limite de tamanho nem sanitização de caracteres de controle/bidi, nem no cliente nem no servidor. `ReportLatency` aceita qualquer `u32`. `add_watcher` aceita `sharer_id` que não é membro/sharer.

## Por que é explorável
Não é XSS (o Leptos escapa texto). Mas: um `nick` de vários MB é armazenado e **retransmitido** a cada membro (`PeerJoined` + snapshot) — amplificação; `room_name` volta sem auth em `/api/rooms/:code`; `\\n`/override RTL/homoglifos em `nick` renderizam na UI dos outros membros, permitindo passar-se por outro membro. `ReportLatency` falsifica o ping exibido de terceiros; `add_watcher` polui `room.watchers`.

## Evidência
`crates/protocol/src/client.rs:9-23`; `apps/web/src/features/home/create_room.rs:43-59` (só `.trim()`); `crates/signaling/src/registry.rs:109-160,311-327,349-366`; render em `apps/web/src/features/room/member_card.rs:249,278`.

## Impacto
Amplificação de broadcast (DoS); falsificação de identidade visual; poluição de estado.

## Sugestão de correção
No servidor (fonte da verdade) + espelho no cliente: `MAX_NICK_LEN` (~32), `MAX_ROOM_NAME_LEN` (~64), `color` restrito à paleta (rejeitar), remover/recusar caracteres de controle e bidi, normalizar NFC. Teto para `ReportLatency`. `add_watcher` só para membros.

## Critérios de aceite
- [ ] `nick`/`room_name` acima do limite => erro dedicado no `CreateRoom`/`JoinRoom`.
- [ ] `color` fora da paleta rejeitado no servidor.
- [ ] `nick` com `\\n`/`\\u202E` recusado ou saneado (teste unitário).
- [ ] `ReportLatency` acima do teto e `WatchShare` para `sharer_id` inexistente são descartados.
""",
    ),
    (
        9,
        "[Segurança] Limitador de brute force de senha confia em fly-client-ip sem validar proxy confiável",
        "security, severity:medium, auth, rate-limit",
        """## Problema
`client_key` usa o header `fly-client-ip` diretamente como chave do balde de tentativas de senha, sem verificar que a requisição veio por um proxy confiável; sem o header, cai numa chave constante `"unknown"`.

## Por que é explorável
Fora da Fly (ou com o app acessível diretamente), o atacante rotaciona `fly-client-ip` a cada conexão e ignora `MAX_PASSWORD_ATTEMPTS` => brute force ilimitado da senha da sala. Com o header ausente, 5 falhas de um atacante trancam todos os usuários daquela sala (DoS).

## Evidência
`crates/signaling/src/ws.rs:12-24`
```rust
fn client_key(headers: &HeaderMap) -> String {
    headers.get("fly-client-ip").and_then(|v| v.to_str().ok())
        .map(str::to_owned).unwrap_or_else(|| "unknown".to_string())
}
```
`crates/signaling/src/registry.rs:429-445`.

## Impacto
Brute force ilimitado de senha (fora da Fly); ou negação de entrada a usuários legítimos.

## Sugestão de correção
Só confiar em `fly-client-ip`/`X-Forwarded-For` quando a conexão vier de `TRUSTED_PROXIES`; senão usar `ConnectInfo<SocketAddr>`. Nunca cair numa chave constante compartilhada — usar o IP de peer real.

## Critérios de aceite
- [ ] Variar `fly-client-ip` a cada request sem proxy confiável NÃO aumenta as tentativas permitidas (teste).
- [ ] Sem header e sem proxy confiável, a chave é o IP de peer real, não `"unknown"`.
- [ ] Config de proxies confiáveis documentada.
""",
    ),
    (
        10,
        "[Segurança] Desktop: renderer remoto sem bloqueio de navegação e pontes IPC sem checagem de origem",
        "security, severity:medium, desktop",
        """## Problema
A `BrowserWindow` principal carrega uma origem **remota** e não há `setWindowOpenHandler` nem handler de `will-navigate`/`will-redirect`. As pontes `desktopAudio`, `picker` e `desktopShare` são expostas a essa janela e nenhum handler `ipcMain` verifica `event.senderFrame`.

## Por que é explorável
Um XSS/open-redirect na origem web (ou sequestro dela) navega o renderer privilegiado para conteúdo do atacante, que então chama:
- `picker.listAudioApps()` / `desktopAudio.start({mode:'screen',excludedBinaries:[]})` — enumera apps em execução e nomes de executáveis, e **inicia captura de áudio do sistema sem consentimento nem seletor**; no Windows o PCM misturado é empurrado ao renderer via `desktop-audio-pcm-chunk` (`platform/windows/audio.ts:26-29`) — captura encoberta exfiltrável.
- `desktopShare.linkReady("...")` — sobrescreve a área de transferência; `memberJoined` — notificação de SO com texto arbitrário.

## Evidência
`desktop/src/main/window.ts:16-36`; `desktop/src/preload.ts:9-49`; `desktop/src/features/audio-share/ipc.ts:12-26`; `desktop/src/platform/windows/audio.ts:19-32`; `desktop/src/features/screen-share/quick-share.ts:11-32`.

## Impacto
Escalada de XSS web para captura encoberta de áudio do sistema, enumeração de processos e sequestro de clipboard no app desktop.

## Sugestão de correção
- `webContents.setWindowOpenHandler(() => ({ action: 'deny' }))`; bloquear `will-navigate`/`will-redirect` para fora da origem do app.
- Fixar `contextIsolation`/`sandbox`/`nodeIntegration:false`/`webSecurity:true` explicitamente.
- Verificar `event.senderFrame` (origem == app) em todo handler `ipcMain`.
- Não emitir `desktop-audio-pcm-chunk` sem uma sessão de compartilhamento iniciada pelo app; validar o formato em `linkReady`.

## Critérios de aceite
- [ ] `window.open`/navegação para `https://evil.com` do renderer são bloqueados (teste `_electron`).
- [ ] Handlers IPC rejeitam `senderFrame` que não seja a origem do app (teste unitário).
- [ ] `desktopAudio.start` sem sessão ativa não emite PCM.
- [ ] `webPreferences` fixa as 4 flags.
""",
    ),
    (
        11,
        "[Segurança] App Axum sem cabeçalhos de segurança / CSP",
        "security, severity:low, hardening",
        """## Problema
Nenhum middleware adiciona `Content-Security-Policy`, `Strict-Transport-Security`, `X-Content-Type-Options`, `Referrer-Policy` ou `Permissions-Policy`.

## Por que é relevante
Defesa em profundidade: uma CSP restritiva conteria um XSS hipotético e limitaria a superfície das pontes desktop (F10/F11); sem HSTS o `force_https` da Fly só redireciona; sem `Permissions-Policy` não há restrição declarada de `display-capture`/`camera`/`microphone`.

## Evidência
`apps/web/src/main.rs:39-56` — router sem `tower-http`. `apps/web/src/app.rs:14-39` — `shell` sem `<meta http-equiv>`.

## Impacto
Menor contenção de XSS e de abuso das pontes desktop.

## Sugestão de correção
`SetResponseHeaderLayer` (ou `tower-helmet`) com CSP (`default-src 'self'`; `connect-src 'self' wss: stun: turn:`; `img-src 'self' data:`; `style-src 'self' fonts.googleapis.com`; `font-src fonts.gstatic.com`), HSTS, `nosniff`, `Referrer-Policy: no-referrer`, `Permissions-Policy` mínimo.

## Critérios de aceite
- [ ] Resposta HTTP inclui os 5 cabeçalhos.
- [ ] App e Google Fonts carregam sem violação de CSP no console.
- [ ] Teste de fumaça (criar/entrar/assistir) passa.
""",
    ),
    (
        12,
        "[Segurança] Higiene de segredos/config: validação de TURN_SECRET, realm, senha em sessionStorage, turns: TLS",
        "security, severity:low, config",
        """## Problema (agrupado)
1. `TurnConfig::from_vars` só rejeita `TURN_SECRET` **vazio** — um placeholder fraco (`changeme`) é aceito silenciosamente. `--realm` cai num default público sem asserção (`crates/signaling/src/turn.rs:31-50`, `docker-entrypoint.sh:11-37`).
2. A senha da sala é persistida em **texto puro** em `sessionStorage` (`apps/web/src/infra/storage.rs:32-64`) — legível por qualquer script na origem.
3. coturn roda com `--no-tls --no-dtls`; `TURN_URLS` usa `turn:` e não `turns:` (`docker-entrypoint.sh:35-37`, `fly.toml:25`).

## Por que é relevante
Segredo TURN fraco não é detectado no boot; XSS/pontes desktop podem ler a senha da sala; o canal de controle TURN trafega em claro e não atravessa firewalls só-TLS.

## Impacto
Baixo, individualmente: robustez de configuração e defesa em profundidade.

## Sugestão de correção
1. Validar no startup: comprimento/entropia mínimos de `TURN_SECRET` + denylist de placeholders; abortar se falhar. `TURN_REALM` obrigatório com TURN ligado.
2. Substituir a senha em `sessionStorage` por um token de rejoin de curta duração emitido pelo servidor (ou aceitar o risco em ADR, com CSP aplicada).
3. Habilitar `turns:` (TLS) na 5349 com certificado; manter `turn:` como fallback.

## Critérios de aceite
- [ ] Processo recusa iniciar com `TURN_SECRET` curto ou em denylist (teste).
- [ ] Rejoin após reload funciona sem gravar a senha em claro, OU ADR de risco aceito + CSP.
- [ ] coturn escuta `turns:` com certificado; `TURN_URLS` inclui a URL `turns:`.
""",
    ),
    (
        13,
        "[Segurança] Higiene informativa: pin de ações de CI, asar, PII no package.json",
        "security, severity:info, hardening",
        """## Problema (agrupado, informativo)
1. `.github/workflows/ci-cd.yml` usa `superfly/flyctl-actions/setup-flyctl@master` (sem pin) — o job tem acesso a `secrets.FLY_API_TOKEN`.
2. `desktop/package.json:52` — `asar: false` (app sem integridade de bundle).
3. `desktop/package.json:5-8` — e-mail pessoal do mantenedor versionado (PII).

## Impacto
Baixo/informativo: risco de cadeia de suprimento no deploy; adulteração local mais fácil; exposição de PII.

## Sugestão de correção
1. Fixar todas as `uses:` de terceiros por SHA de commit.
2. `asar: true` + `asarIntegrity`.
3. Usar um e-mail de projeto/alias.

## Critérios de aceite
- [ ] Ações de Cit de terceiros fixadas por SHA.
- [ ] Build empacotado usa `asar` com verificação de integridade.
- [ ] `author.email` aponta para endereço de projeto.
""",
    ),
]


# ---------------------------------------------------------------------------
# Gráficos
# ---------------------------------------------------------------------------
def build_charts() -> tuple[str, str]:
    os.makedirs(CHART_DIR, exist_ok=True)

    counts = {s: 0 for s in SEV_ORDER}
    for f in FINDINGS:
        counts[f["sev"]] += 1

    # Donut por severidade
    labels = [s for s in SEV_ORDER if counts[s] > 0]
    sizes = [counts[s] for s in labels]
    pie_colors = [SEV_COLOR[s] for s in labels]

    fig, ax = plt.subplots(figsize=(4.6, 3.4), dpi=200)
    wedges, _texts, autotexts = ax.pie(
        sizes,
        colors=pie_colors,
        startangle=90,
        counterclock=False,
        autopct=lambda p: f"{round(p * sum(sizes) / 100)}",
        pctdistance=0.78,
        wedgeprops=dict(width=0.42, edgecolor="white", linewidth=1.5),
    )
    for t in autotexts:
        t.set_color("white")
        t.set_fontsize(9)
        t.set_fontweight("bold")
    ax.legend(
        wedges,
        [f"{s} ({counts[s]})" for s in labels],
        loc="center left",
        bbox_to_anchor=(1.0, 0.5),
        frameon=False,
        fontsize=8.5,
    )
    ax.set_title("Achados por severidade", fontsize=10, fontweight="bold")
    ax.axis("equal")
    donut_path = os.path.join(CHART_DIR, "donut_severidade.png")
    fig.savefig(donut_path, bbox_inches="tight", facecolor="white")
    plt.close(fig)

    # Barras por categoria (empilhado por severidade)
    cat_sev = {c: {s: 0 for s in SEV_ORDER} for c in CATEGORIES}
    for f in FINDINGS:
        cat_sev[f["cat"]][f["sev"]] += 1
    cats_present = [c for c in CATEGORIES if sum(cat_sev[c].values()) > 0]

    fig, ax = plt.subplots(figsize=(7.4, 4.2), dpi=200)
    left = [0] * len(cats_present)
    yidx = list(range(len(cats_present)))
    for s in SEV_ORDER:
        vals = [cat_sev[c][s] for c in cats_present]
        if not any(vals):
            continue
        ax.barh(yidx, vals, left=left, color=SEV_COLOR[s], label=s, height=0.62)
        left = [l + v for l, v in zip(left, vals)]
    ax.set_yticks(yidx)
    ax.set_yticklabels(cats_present, fontsize=8)
    ax.invert_yaxis()
    ax.set_xlabel("Nº de achados", fontsize=8.5)
    ax.set_title("Achados por categoria", fontsize=10, fontweight="bold")
    ax.legend(frameon=False, fontsize=7.5, ncol=5, loc="upper center", bbox_to_anchor=(0.5, -0.16))
    maxx = max(sum(cat_sev[c].values()) for c in cats_present)
    ax.set_xticks(range(0, maxx + 1))
    ax.grid(axis="x", color="#E5E7EB", linewidth=0.6)
    ax.set_axisbelow(True)
    for spine in ("top", "right"):
        ax.spines[spine].set_visible(False)
    bar_path = os.path.join(CHART_DIR, "barras_categoria.png")
    fig.savefig(bar_path, bbox_inches="tight", facecolor="white")
    plt.close(fig)

    return donut_path, bar_path


# ---------------------------------------------------------------------------
# PDF
# ---------------------------------------------------------------------------
PAGE_W, PAGE_H = A4
MARGIN = 2 * cm


def wrap_mono(text: str, width: int) -> str:
    """Hard-wrap each line of a monospaced block so nothing runs past the
    page margin (Preformatted does not wrap on its own). Continuation lines
    get a 2-space hanging indent so wrapped code stays readable."""
    out = []
    for line in text.split("\n"):
        if len(line) <= width:
            out.append(line)
            continue
        stripped = line.lstrip(" ")
        base_indent = line[: len(line) - len(stripped)]
        wrapped = textwrap.wrap(
            stripped,
            width=width - len(base_indent),
            break_long_words=True,
            break_on_hyphens=False,
            subsequent_indent="  ",
        )
        out.extend(base_indent + w for w in (wrapped or [""]))
    return "\n".join(out)


def _footer(canvas, doc):
    canvas.saveState()
    canvas.setFont("Helvetica", 7.5)
    canvas.setFillColor(colors.HexColor("#6B7280"))
    canvas.drawString(MARGIN, 1.1 * cm, REPORT_NAME)
    canvas.drawRightString(PAGE_W - MARGIN, 1.1 * cm, f"Página {doc.page}")
    canvas.setStrokeColor(colors.HexColor("#E5E7EB"))
    canvas.line(MARGIN, 1.45 * cm, PAGE_W - MARGIN, 1.45 * cm)
    canvas.restoreState()


def _styles():
    ss = getSampleStyleSheet()
    styles = {
        "h1": ParagraphStyle("h1", parent=ss["Heading1"], fontSize=17, spaceBefore=6,
                             spaceAfter=10, textColor=colors.HexColor("#111827")),
        "h2": ParagraphStyle("h2", parent=ss["Heading2"], fontSize=12.5, spaceBefore=14,
                             spaceAfter=6, textColor=colors.HexColor("#1F2937")),
        "h3": ParagraphStyle("h3", parent=ss["Heading3"], fontSize=10.5, spaceBefore=8,
                             spaceAfter=3, textColor=colors.HexColor("#374151")),
        "body": ParagraphStyle("body", parent=ss["BodyText"], fontSize=9, leading=13,
                               alignment=TA_JUSTIFY, spaceAfter=5),
        "small": ParagraphStyle("small", parent=ss["BodyText"], fontSize=8, leading=11,
                                textColor=colors.HexColor("#4B5563")),
        "cell": ParagraphStyle("cell", parent=ss["BodyText"], fontSize=7.8, leading=10.5),
        "cellb": ParagraphStyle("cellb", parent=ss["BodyText"], fontSize=7.8, leading=10.5,
                                fontName="Helvetica-Bold"),
        "code": ParagraphStyle("code", parent=ss["Code"], fontSize=7, leading=9,
                               textColor=colors.HexColor("#111827"),
                               backColor=colors.HexColor("#F3F4F6")),
        "cover_title": ParagraphStyle("ct", parent=ss["Title"], fontSize=23, leading=28,
                                      alignment=TA_CENTER, textColor=colors.HexColor("#111827")),
        "cover_sub": ParagraphStyle("cs", parent=ss["BodyText"], fontSize=10.5, leading=15,
                                    alignment=TA_CENTER, textColor=colors.HexColor("#374151")),
        "issue": ParagraphStyle("issue", parent=ss["Code"], fontSize=6.8, leading=8.6,
                                textColor=colors.HexColor("#111827")),
    }
    return styles


def sev_chip(sev: str, st) -> Table:
    t = Table([[sev]], colWidths=[2.1 * cm], rowHeights=[0.5 * cm])
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), colors.HexColor(SEV_COLOR[sev])),
        ("TEXTCOLOR", (0, 0), (-1, -1), colors.white),
        ("FONTNAME", (0, 0), (-1, -1), "Helvetica-Bold"),
        ("FONTSIZE", (0, 0), (-1, -1), 7.5),
        ("ALIGN", (0, 0), (-1, -1), "CENTER"),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
    ]))
    return t


def build_pdf():
    donut_path, bar_path = build_charts()
    st = _styles()

    doc = BaseDocTemplate(
        OUT_PDF, pagesize=A4,
        leftMargin=MARGIN, rightMargin=MARGIN, topMargin=MARGIN, bottomMargin=2 * cm,
        title=REPORT_NAME, author="Auditoria de Segurança",
    )
    frame = Frame(MARGIN, 2 * cm, PAGE_W - 2 * MARGIN, PAGE_H - MARGIN - 2 * cm, id="main")
    cover_frame = Frame(MARGIN, 2 * cm, PAGE_W - 2 * MARGIN, PAGE_H - MARGIN - 2 * cm, id="cover")
    doc.addPageTemplates([
        PageTemplate(id="cover", frames=[cover_frame]),
        PageTemplate(id="main", frames=[frame], onPage=_footer),
    ])

    story = []

    # ---- Capa ----
    story.append(Spacer(1, 3.2 * cm))
    story.append(Paragraph("Relatório de Auditoria de Segurança", st["cover_title"]))
    story.append(Paragraph("Screen Share", st["cover_title"]))
    story.append(Spacer(1, 0.8 * cm))
    story.append(HRFlowable(width="40%", thickness=1.2, color=colors.HexColor("#B91C1C")))
    story.append(Spacer(1, 0.8 * cm))
    story.append(Paragraph(f"Data: {AUDIT_DATE}", st["cover_sub"]))
    story.append(Spacer(1, 0.3 * cm))
    story.append(Paragraph(
        "Escopo auditado: <b>crates/protocol</b>, <b>crates/signaling</b> (relay Axum/WebSocket + "
        "registry em memória + TURN), <b>apps/web</b> (Leptos SSR + hydrate/WASM), <b>desktop/</b> "
        "(Electron), e artefatos de implantação (<b>Dockerfile</b>, <b>docker-entrypoint.sh</b>, "
        "<b>fly.toml</b>, <b>.github/workflows</b>). Revisão de código estática, manual, arquivo por "
        "arquivo; histórico git varrido por segredos. Sem teste dinâmico contra ambiente vivo.",
        st["cover_sub"]))
    story.append(Spacer(1, 0.7 * cm))
    story.append(Paragraph(
        "<b>Nota metodológica.</b> As cinco categorias do roteiro foram mapeadas para a stack: "
        "(1) <i>isolamento de inquilino</i> → não há banco/ORM; o mecanismo é o <i>registry</i> em "
        "memória chaveado por código de sala, com <i>room_code</i> + <i>peer_id</i> vinculados à "
        "conexão no servidor. (2) <i>Permissão no navegador</i> → o modelo de sala é plano (todo "
        "membro é igual, sem papel de admin/host), então quase não há operação privilegiada a "
        "verificar; a única checagem que importa (senha da sala) é feita no servidor. (3) <i>IDOR</i> "
        "→ IDs de objeto são o código de sala (rota <i>/api/rooms/:code</i> e <i>JoinRoom</i>) e os "
        "<i>peer_id</i> nas mensagens de sinalização. (4) <i>Segredos</i> → <i>Dockerfile</i>, "
        "<i>docker-entrypoint.sh</i>, <i>fly.toml</i>, CI, bundle do frontend e histórico git. "
        "(5) <i>Entrada não tratada</i> → no frontend, Leptos escapa por padrão e não há "
        "<i>inner_html</i>/<i>eval</i>; foco em CSS/atributos, <i>spoofing</i> por Unicode e injeção "
        "de comando no desktop.",
        st["cover_sub"]))

    story.append(NextPageTemplate("main"))
    story.append(PageBreak())

    # ---- Resumo executivo ----
    counts = {s: 0 for s in SEV_ORDER}
    for f in FINDINGS:
        counts[f["sev"]] += 1
    total = len(FINDINGS)

    story.append(Paragraph("1. Resumo executivo", st["h1"]))
    story.append(Paragraph(
        f"A auditoria registrou <b>{total} achados verificados</b> mais <b>{len(STRENGTHS)} pontos "
        f"fortes</b> comprovados. Distribuição por severidade: "
        f"<b>{counts['CRÍTICA']}</b> crítica, <b>{counts['ALTA']}</b> alta, "
        f"<b>{counts['MÉDIA']}</b> média, <b>{counts['BAIXA']}</b> baixa, "
        f"<b>{counts['INFORMATIVA']}</b> informativa.",
        st["body"]))
    story.append(Paragraph(
        "O risco central não está no vídeo P2P (protegido por DTLS-SRTP) e sim na "
        "<b>infraestrutura de relay TURN</b> (SSRF a metadados de nuvem e abuso de banda — F01), "
        "na <b>ausência de limites de recurso</b> no relay de sinalização (DoS remoto trivial e "
        "não autenticado — F02/F03) e na <b>cadeia de suprimento do app desktop</b> "
        "(auto-update sem verificação de assinatura — F04). Nenhum achado é uma violação direta "
        "de dados/RCE remota sem pré-condição — não há banco de dados nem funções de servidor "
        "além de <i>/ws</i> e <i>/api/rooms/:code</i>.",
        st["body"]))

    imgs = Table(
        [[Image(donut_path, width=8.0 * cm, height=5.9 * cm),
          Image(bar_path, width=8.4 * cm, height=4.8 * cm)]],
        colWidths=[8.3 * cm, 8.7 * cm],
    )
    imgs.setStyle(TableStyle([("VALIGN", (0, 0), (-1, -1), "MIDDLE")]))
    story.append(Spacer(1, 4))
    story.append(imgs)

    # ---- Pontos fortes ----
    story.append(Paragraph("2. Pontos fortes (o que está protegido)", st["h1"]))
    for sgood in STRENGTHS:
        story.append(Paragraph(f"&#10003; {sgood['title']}", st["h3"]))
        story.append(Paragraph(sgood["ev"], st["small"]))
    story.append(Spacer(1, 4))

    # ---- Pontos fracos ----
    story.append(Paragraph("3. Pontos fracos (riscos centrais)", st["h1"]))
    weak_rows = [[Paragraph("<b>ID</b>", st["cell"]), Paragraph("<b>Sev.</b>", st["cell"]),
                 Paragraph("<b>Risco</b>", st["cell"])]]
    for f in FINDINGS:
        if f["sev"] in ("CRÍTICA", "ALTA", "MÉDIA"):
            weak_rows.append([
                Paragraph(f["id"], st["cellb"]),
                sev_chip(f["sev"], st),
                Paragraph(f["title"], st["cell"]),
            ])
    wt = Table(weak_rows, colWidths=[1.1 * cm, 2.3 * cm, 13.2 * cm])
    wt.setStyle(TableStyle([
        ("GRID", (0, 0), (-1, -1), 0.4, colors.HexColor("#D1D5DB")),
        ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#F3F4F6")),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("TOPPADDING", (0, 0), (-1, -1), 3),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 3),
    ]))
    story.append(wt)

    # ---- Achados detalhados ----
    story.append(PageBreak())
    story.append(Paragraph("4. Achados detalhados por categoria", st["h1"]))

    by_cat: dict[str, list] = {}
    for f in FINDINGS:
        by_cat.setdefault(f["cat"], []).append(f)

    sev_rank = {s: i for i, s in enumerate(SEV_ORDER)}
    for cat in CATEGORIES:
        if cat not in by_cat:
            continue
        story.append(Paragraph(cat, st["h2"]))
        for f in sorted(by_cat[cat], key=lambda x: sev_rank[x["sev"]]):
            header = Table(
                [[sev_chip(f["sev"], st),
                  Paragraph(f"<b>{f['id']} — {f['title']}</b>", st["cell"])]],
                colWidths=[2.3 * cm, 13.3 * cm],
            )
            header.setStyle(TableStyle([
                ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 2),
            ]))
            story.append(header)
            story.append(Paragraph("<b>Arquivo:linha:</b> " + "; ".join(f["loc"]), st["small"]))
            story.append(Preformatted(wrap_mono(f["code"], 100), st["code"]))
            story.append(Paragraph("<b>Por que é explorável:</b> " + f["why"], st["small"]))
            story.append(Paragraph("<b>Impacto:</b> " + f["impact"], st["small"]))
            story.append(Paragraph("<b>Correção sugerida:</b> " + f["fix"], st["small"]))
            story.append(Paragraph("<b>Critérios de aceite:</b>", st["small"]))
            for a in f["accept"]:
                story.append(Paragraph(f"&#9744; {a}", st["small"]))
            story.append(Spacer(1, 8))

    # ---- Recomendações priorizadas ----
    story.append(PageBreak())
    story.append(Paragraph("5. Recomendações priorizadas", st["h1"]))
    rec_rows = [[Paragraph("<b>Prio.</b>", st["cell"]), Paragraph("<b>Ação</b>", st["cell"])]]
    for r in RECOMMENDATIONS:
        rec_rows.append([Paragraph(f"<b>{r['p']}</b>", st["cellb"]), Paragraph(r["text"], st["cell"])])
    rt = Table(rec_rows, colWidths=[1.6 * cm, 14.9 * cm])
    rt.setStyle(TableStyle([
        ("GRID", (0, 0), (-1, -1), 0.4, colors.HexColor("#D1D5DB")),
        ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#F3F4F6")),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("TOPPADDING", (0, 0), (-1, -1), 4),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
        ("BACKGROUND", (0, 1), (0, -1), colors.HexColor("#FAFAFA")),
    ]))
    story.append(rt)

    # ---- Issues para o GitHub ----
    story.append(PageBreak())
    story.append(Paragraph("6. Issues para o GitHub", st["h1"]))
    story.append(Paragraph(
        "Cada bloco abaixo é o texto completo de uma issue em Markdown, pronto para copiar e "
        "colar. Achados triviais relacionados foram agrupados numa issue única.",
        st["body"]))
    for num, title, labels, bodymd in GH_ISSUES:
        block = (
            f"--- ISSUE {num} ---\n\n"
            f"Título: {title}\n\n"
            f"Labels sugeridas: {labels}\n\n"
            f"{bodymd.strip()}\n\n"
            f"--- FIM ISSUE {num} ---"
        )
        story.append(Preformatted(wrap_mono(block, 112), st["issue"]))
        story.append(Spacer(1, 10))

    doc.build(story)


# ---------------------------------------------------------------------------
# HTML (arquivo único, offline, sem dependências externas)
# ---------------------------------------------------------------------------
def _b64_png(path: str) -> str:
    with open(path, "rb") as fh:
        return "data:image/png;base64," + base64.b64encode(fh.read()).decode("ascii")


def _md_inline(text: str) -> str:
    """Escapa HTML e converte `trecho` em <code>trecho</code>."""
    esc = _html.escape(text, quote=False)
    return re.sub(r"`([^`]+)`", lambda m: f"<code>{m.group(1)}</code>", esc)


def _chip(sev: str) -> str:
    return f'<span class="chip" style="background:{SEV_COLOR[sev]}">{sev}</span>'


def build_html() -> None:
    donut_path, bar_path = build_charts()
    donut_b64 = _b64_png(donut_path)
    bar_b64 = _b64_png(bar_path)

    counts = {s: 0 for s in SEV_ORDER}
    for f in FINDINGS:
        counts[f["sev"]] += 1
    total = len(FINDINGS)

    css = """
:root{
  --bg:#ffffff; --fg:#111827; --muted:#4b5563; --line:#e5e7eb;
  --card:#ffffff; --code-bg:#f3f4f6; --accent:#b91c1c;
}
@media (prefers-color-scheme: dark){
  :root{
    --bg:#0f1115; --fg:#e5e7eb; --muted:#9ca3af; --line:#2a2f3a;
    --card:#151922; --code-bg:#1b212c; --accent:#f87171;
  }
  img.chart{background:#fff;border-radius:8px}
}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{
  margin:0;background:var(--bg);color:var(--fg);
  font:15px/1.55 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;
}
.wrap{max-width:960px;margin:0 auto;padding:32px 24px 80px}
h1{font-size:1.9rem;margin:2.2rem 0 .6rem;letter-spacing:-.01em}
h2{font-size:1.25rem;margin:2rem 0 .5rem;padding-bottom:.3rem;border-bottom:1px solid var(--line)}
h3{font-size:1rem;margin:1.1rem 0 .3rem}
p{margin:.5rem 0}
a{color:inherit}
code{background:var(--code-bg);padding:.08em .35em;border-radius:4px;font-size:.86em;
  font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
small,.muted{color:var(--muted)}
.cover{border:1px solid var(--line);border-left:4px solid var(--accent);
  border-radius:10px;padding:22px 24px;margin-top:8px;background:var(--card)}
.cover .t{font-size:1.7rem;font-weight:700;letter-spacing:-.01em}
.cover .sub{color:var(--muted);font-size:.92rem;margin-top:.5rem}
.chip{display:inline-block;color:#fff;font-weight:700;font-size:.68rem;
  letter-spacing:.03em;padding:.18em .6em;border-radius:999px;vertical-align:middle}
.kpis{display:flex;flex-wrap:wrap;gap:10px;margin:14px 0}
.kpi{flex:1 1 120px;border:1px solid var(--line);border-radius:10px;padding:10px 12px;background:var(--card)}
.kpi b{display:block;font-size:1.5rem;line-height:1}
.kpi span{color:var(--muted);font-size:.75rem}
.charts{display:flex;flex-wrap:wrap;gap:20px;align-items:center;justify-content:center;margin:18px 0}
img.chart{max-width:100%;height:auto}
table{border-collapse:collapse;width:100%;margin:12px 0;font-size:.9rem}
th,td{border:1px solid var(--line);padding:7px 9px;text-align:left;vertical-align:top}
th{background:var(--code-bg);font-size:.8rem;text-transform:uppercase;letter-spacing:.03em}
.finding{border:1px solid var(--line);border-radius:10px;padding:14px 16px;margin:14px 0;background:var(--card)}
.finding .fh{display:flex;gap:10px;align-items:baseline;flex-wrap:wrap;margin-bottom:.3rem}
.finding .fh .id{font-weight:700}
.finding .loc{color:var(--muted);font-size:.8rem;margin:.2rem 0 .5rem}
pre{background:var(--code-bg);border:1px solid var(--line);border-radius:8px;
  padding:10px 12px;overflow-x:auto;font-size:.8rem;line-height:1.45;
  font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;
  white-space:pre-wrap;word-break:break-word}
.finding .lbl{font-weight:700;font-size:.82rem}
.acc{list-style:none;padding-left:0;margin:.3rem 0}
.acc li::before{content:"\\2610  ";color:var(--muted)}
.toc{border:1px solid var(--line);border-radius:10px;padding:12px 16px;background:var(--card);font-size:.9rem}
.toc ol{margin:.3rem 0;padding-left:1.3rem}
.issue-block{position:relative}
.issue-block button{position:absolute;top:8px;right:8px;font:inherit;font-size:.72rem;
  padding:.25em .7em;border:1px solid var(--line);border-radius:6px;background:var(--bg);
  color:var(--fg);cursor:pointer}
.issue-block button:hover{background:var(--code-bg)}
hr{border:0;border-top:1px solid var(--line);margin:2rem 0}
@media print{
  a[href]::after{content:""}
  .issue-block button{display:none}
  .finding,pre,table{page-break-inside:avoid}
  body{font-size:11pt}
  .wrap{max-width:none;padding:0}
}
"""

    parts: list[str] = []
    parts.append("<!doctype html><html lang='pt-BR'><head><meta charset='utf-8'>")
    parts.append("<meta name='viewport' content='width=device-width,initial-scale=1'>")
    parts.append(f"<title>{_html.escape(REPORT_NAME)}</title>")
    parts.append(f"<style>{css}</style></head><body><div class='wrap'>")

    # Capa
    parts.append(
        "<div class='cover'>"
        "<div class='t'>Relatório de Auditoria de Segurança &mdash; Screen Share</div>"
        f"<div class='sub'><b>Data:</b> {AUDIT_DATE}</div>"
        "<div class='sub'><b>Escopo auditado:</b> <code>crates/protocol</code>, "
        "<code>crates/signaling</code> (relay Axum/WebSocket + registry em memória + TURN), "
        "<code>apps/web</code> (Leptos SSR + hydrate/WASM), <code>desktop/</code> (Electron), "
        "e artefatos de implantação (<code>Dockerfile</code>, <code>docker-entrypoint.sh</code>, "
        "<code>fly.toml</code>, <code>.github/workflows</code>). Revisão de código estática, manual, "
        "arquivo por arquivo; histórico git varrido por segredos. Sem teste dinâmico contra "
        "ambiente vivo.</div>"
        "<div class='sub'><b>Nota metodológica.</b> As cinco categorias do roteiro foram mapeadas "
        "para a stack: (1) <i>isolamento de inquilino</i> &rarr; sem banco/ORM; o mecanismo &eacute; "
        "o <i>registry</i> em mem&oacute;ria chaveado por c&oacute;digo de sala, com "
        "<code>room_code</code> + <code>peer_id</code> vinculados &agrave; conex&atilde;o no "
        "servidor. (2) <i>Permiss&atilde;o no navegador</i> &rarr; modelo de sala plano (sem papel "
        "de admin/host), ent&atilde;o quase n&atilde;o h&aacute; opera&ccedil;&atilde;o "
        "privilegiada a verificar; a &uacute;nica checagem que importa (senha) &eacute; feita no "
        "servidor. (3) <i>IDOR</i> &rarr; IDs de objeto s&atilde;o o c&oacute;digo de sala "
        "(<code>/api/rooms/:code</code> e <code>JoinRoom</code>) e os <code>peer_id</code> na "
        "sinaliza&ccedil;&atilde;o. (4) <i>Segredos</i> &rarr; Dockerfile, entrypoint, "
        "<code>fly.toml</code>, CI, bundle do frontend e hist&oacute;rico git. (5) <i>Entrada "
        "n&atilde;o tratada</i> &rarr; no frontend, Leptos escapa por padr&atilde;o e n&atilde;o "
        "h&aacute; <code>inner_html</code>/<code>eval</code>; foco em CSS/atributos, <i>spoofing</i> "
        "por Unicode e inje&ccedil;&atilde;o de comando no desktop.</div>"
        "</div>"
    )

    # TOC
    parts.append(
        "<div class='toc'><b>Conte&uacute;do</b><ol>"
        "<li><a href='#resumo'>Resumo executivo</a></li>"
        "<li><a href='#fortes'>Pontos fortes</a></li>"
        "<li><a href='#fracos'>Pontos fracos</a></li>"
        "<li><a href='#detalhes'>Achados detalhados por categoria</a></li>"
        "<li><a href='#recs'>Recomenda&ccedil;&otilde;es priorizadas</a></li>"
        "<li><a href='#issues'>Issues para o GitHub</a></li>"
        "</ol></div>"
    )

    # 1. Resumo executivo
    parts.append("<h1 id='resumo'>1. Resumo executivo</h1>")
    parts.append("<div class='kpis'>")
    for s in SEV_ORDER:
        parts.append(
            f"<div class='kpi' style='border-left:4px solid {SEV_COLOR[s]}'>"
            f"<b>{counts[s]}</b><span>{s}</span></div>"
        )
    good_color = SEV_COLOR["PONTO FORTE"]
    parts.append(
        f"<div class='kpi' style='border-left:4px solid {good_color}'>"
        f"<b>{len(STRENGTHS)}</b><span>PONTOS FORTES</span></div></div>"
    )
    parts.append(
        f"<p>A auditoria registrou <b>{total} achados verificados</b> mais "
        f"<b>{len(STRENGTHS)} pontos fortes</b> comprovados.</p>"
        "<p>O risco central n&atilde;o est&aacute; no v&iacute;deo P2P (protegido por DTLS-SRTP) e "
        "sim na <b>infraestrutura de relay TURN</b> (SSRF a metadados de nuvem e abuso de banda "
        "&mdash; F01), na <b>aus&ecirc;ncia de limites de recurso</b> no relay de "
        "sinaliza&ccedil;&atilde;o (DoS remoto trivial e n&atilde;o autenticado &mdash; F02/F03) e "
        "na <b>cadeia de suprimento do app desktop</b> (auto-update sem verifica&ccedil;&atilde;o de "
        "assinatura &mdash; F04). Nenhum achado &eacute; uma viola&ccedil;&atilde;o direta de "
        "dados/RCE remota sem pr&eacute;-condi&ccedil;&atilde;o &mdash; n&atilde;o h&aacute; banco "
        "de dados nem fun&ccedil;&otilde;es de servidor al&eacute;m de <code>/ws</code> e "
        "<code>/api/rooms/:code</code>.</p>"
    )
    parts.append(
        "<div class='charts'>"
        f"<img class='chart' alt='Achados por severidade' src='{donut_b64}' style='max-width:380px'>"
        f"<img class='chart' alt='Achados por categoria' src='{bar_b64}' style='max-width:520px'>"
        "</div>"
    )

    # 2. Pontos fortes
    parts.append("<h1 id='fortes'>2. Pontos fortes (o que est&aacute; protegido)</h1>")
    for sgood in STRENGTHS:
        parts.append(f"<h3>&#10003; {_md_inline(sgood['title'])}</h3>")
        parts.append(f"<p class='muted'>{_md_inline(sgood['ev'])}</p>")

    # 3. Pontos fracos
    parts.append("<h1 id='fracos'>3. Pontos fracos (riscos centrais)</h1>")
    parts.append("<table><tr><th>ID</th><th>Sev.</th><th>Risco</th></tr>")
    for f in FINDINGS:
        if f["sev"] in ("CRÍTICA", "ALTA", "MÉDIA"):
            parts.append(
                f"<tr><td><b>{f['id']}</b></td><td>{_chip(f['sev'])}</td>"
                f"<td>{_md_inline(f['title'])}</td></tr>"
            )
    parts.append("</table>")

    # 4. Achados detalhados
    parts.append("<h1 id='detalhes'>4. Achados detalhados por categoria</h1>")
    by_cat: dict[str, list] = {}
    for f in FINDINGS:
        by_cat.setdefault(f["cat"], []).append(f)
    sev_rank = {s: i for i, s in enumerate(SEV_ORDER)}
    for cat in CATEGORIES:
        if cat not in by_cat:
            continue
        parts.append(f"<h2>{_html.escape(cat)}</h2>")
        for f in sorted(by_cat[cat], key=lambda x: sev_rank[x["sev"]]):
            parts.append("<div class='finding'>")
            parts.append(
                f"<div class='fh'>{_chip(f['sev'])}"
                f"<span class='id'>{f['id']} &mdash; {_md_inline(f['title'])}</span></div>"
            )
            parts.append(
                "<div class='loc'><b>Arquivo:linha:</b> "
                + "; ".join(_html.escape(x) for x in f["loc"])
                + "</div>"
            )
            parts.append(f"<pre>{_html.escape(f['code'])}</pre>")
            parts.append(
                f"<p><span class='lbl'>Por que &eacute; explor&aacute;vel:</span> "
                f"{_md_inline(f['why'])}</p>"
            )
            parts.append(
                f"<p><span class='lbl'>Impacto:</span> {_md_inline(f['impact'])}</p>"
            )
            parts.append(
                f"<p><span class='lbl'>Corre&ccedil;&atilde;o sugerida:</span> "
                f"{_md_inline(f['fix'])}</p>"
            )
            parts.append("<p class='lbl'>Crit&eacute;rios de aceite:</p><ul class='acc'>")
            for a in f["accept"]:
                parts.append(f"<li>{_md_inline(a)}</li>")
            parts.append("</ul></div>")

    # 5. Recomendações
    parts.append("<h1 id='recs'>5. Recomenda&ccedil;&otilde;es priorizadas</h1>")
    parts.append("<table><tr><th>Prio.</th><th>A&ccedil;&atilde;o</th></tr>")
    for r in RECOMMENDATIONS:
        parts.append(
            f"<tr><td><b>{r['p']}</b></td><td>{_md_inline(r['text'])}</td></tr>"
        )
    parts.append("</table>")

    # 6. Issues para o GitHub
    parts.append("<h1 id='issues'>6. Issues para o GitHub</h1>")
    parts.append(
        "<p>Cada bloco abaixo &eacute; o texto completo de uma issue em Markdown, pronto para "
        "copiar e colar. Achados triviais relacionados foram agrupados numa issue &uacute;nica.</p>"
    )
    for num, title, labels, bodymd in GH_ISSUES:
        block = (
            f"--- ISSUE {num} ---\n\n"
            f"Título: {title}\n\n"
            f"Labels sugeridas: {labels}\n\n"
            f"{bodymd.strip()}\n\n"
            f"--- FIM ISSUE {num} ---"
        )
        parts.append(
            "<div class='issue-block'>"
            "<button type='button' onclick=\"navigator.clipboard.writeText("
            "this.nextElementSibling.textContent).then(()=>{this.textContent='copiado';"
            "setTimeout(()=>this.textContent='copiar',1500)})\">copiar</button>"
            f"<pre>{_html.escape(block)}</pre></div>"
        )

    parts.append("<hr><p class='muted'>Gerado por "
                 "<code>docs/security-audit/generate_report.py</code>.</p>")
    parts.append("</div></body></html>")

    with open(OUT_HTML, "w", encoding="utf-8") as fh:
        fh.write("".join(parts))


ARTIFACT_CSS = """
/* Fontes do projeto auditado (docs/decisions/0006-visual-redesign.md):
   Space Grotesk display + Space Mono para dados; Libre Franklin para
   texto corrido, denso e legível. */
:root{
  --paper:#f5f6f8; --surface:#ffffff; --ink:#171b21; --muted:#59616e;
  --line:#e2e5ea; --accent:#b4141c; --figure-bg:#ffffff; --code-bg:#eef1f4;
  --sev-critica:#B91C1C; --sev-alta:#EA580C; --sev-media:#D97706;
  --sev-baixa:#2563EB; --sev-info:#64748B; --sev-forte:#059669;
  color-scheme: light dark;
}
@media (prefers-color-scheme: dark){
  :root:not([data-theme="light"]){
    --paper:#0d1014; --surface:#161a21; --ink:#e7e9ee; --muted:#98a2b3;
    --line:#262c36; --accent:#f3767c; --figure-bg:#f3f4f6; --code-bg:#1b212c;
  }
}
:root[data-theme="dark"]{
  --paper:#0d1014; --surface:#161a21; --ink:#e7e9ee; --muted:#98a2b3;
  --line:#262c36; --accent:#f3767c; --figure-bg:#f3f4f6; --code-bg:#1b212c;
}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
@media (prefers-reduced-motion:reduce){html{scroll-behavior:auto}}
body{
  margin:0; background:var(--paper); color:var(--ink);
  font-family:"Libre Franklin",-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;
  font-size:15px; line-height:1.6; -webkit-font-smoothing:antialiased;
}
.page{max-width:900px;margin:0 auto;padding:48px 24px 96px}
.col{max-width:68ch}
h1,h2,h3{font-family:"Space Grotesk",Georgia,sans-serif;text-wrap:balance;line-height:1.2}
h1{font-size:2.4rem;font-weight:700;letter-spacing:-.02em;margin:.2rem 0}
h2{font-size:1.35rem;font-weight:700;margin:2.6rem 0 .9rem;padding-top:1.1rem;border-top:1px solid var(--line)}
h2 .num{font-family:"Space Mono",monospace;color:var(--accent);font-size:.85rem;
  font-weight:700;vertical-align:.35em;margin-right:.6em}
h3{font-size:1.02rem;font-weight:500;margin:1.3rem 0 .3rem}
p{margin:.55rem 0}
a{color:var(--accent);text-underline-offset:2px}
:focus-visible{outline:2px solid var(--accent);outline-offset:2px;border-radius:3px}
.eyebrow{font-family:"Space Mono",monospace;font-size:.72rem;font-weight:700;
  letter-spacing:.22em;text-transform:uppercase;color:var(--muted)}
.rule{height:3px;width:64px;background:var(--accent);margin:1.1rem 0 1.4rem}
.masthead{margin-bottom:1.4rem}
.meta{display:grid;grid-template-columns:max-content 1fr;gap:.35rem 1.2rem;
  font-size:.86rem;margin-top:1rem}
.meta dt{font-family:"Space Mono",monospace;text-transform:uppercase;font-size:.68rem;
  letter-spacing:.12em;color:var(--muted);padding-top:.15rem}
.meta dd{margin:0}
.note{font-size:.85rem;color:var(--muted);border-left:2px solid var(--line);
  padding:.2rem 0 .2rem 1rem;margin:1.4rem 0}
code,.ref{font-family:"Space Mono","SF Mono",Menlo,Consolas,monospace}
code{background:var(--code-bg);padding:.06em .34em;border-radius:4px;font-size:.84em}
.muted{color:var(--muted)}
/* tiles de severidade */
.tally{display:flex;flex-wrap:wrap;gap:10px;margin:1.4rem 0}
.tile{flex:1 1 116px;background:var(--surface);border:1px solid var(--line);
  border-radius:10px;padding:12px 14px;position:relative;overflow:hidden}
.tile::before{content:"";position:absolute;left:0;top:0;bottom:0;width:4px;background:var(--c)}
.tile b{display:block;font-family:"Space Grotesk",sans-serif;font-size:1.7rem;
  font-weight:700;line-height:1;font-variant-numeric:tabular-nums}
.tile span{font-family:"Space Mono",monospace;font-size:.66rem;letter-spacing:.1em;
  text-transform:uppercase;color:var(--muted)}
/* figuras */
.figure{background:var(--figure-bg);border:1px solid var(--line);border-radius:12px;
  padding:16px;margin:1.2rem 0;overflow-x:auto}
.figure img{display:block;max-width:100%;height:auto;margin:0 auto}
.figure figcaption{font-family:"Space Mono",monospace;font-size:.7rem;letter-spacing:.08em;
  text-transform:uppercase;color:#5a6472;margin-top:.7rem;text-align:center}
.figrow{display:flex;flex-wrap:wrap;gap:16px}
.figrow .figure{flex:1 1 300px;margin:0}
/* chip de severidade */
.chip{display:inline-block;color:#fff;font-family:"Space Mono",monospace;font-weight:700;
  font-size:.62rem;letter-spacing:.08em;padding:.28em .7em;border-radius:999px;
  text-transform:uppercase;white-space:nowrap;vertical-align:middle}
/* tabelas */
.tbl{width:100%;border-collapse:collapse;margin:1rem 0;font-size:.9rem}
.tbl th,.tbl td{border:1px solid var(--line);padding:8px 10px;text-align:left;vertical-align:top}
.tbl th{background:var(--code-bg);font-family:"Space Mono",monospace;font-size:.68rem;
  letter-spacing:.08em;text-transform:uppercase;font-weight:700}
.tbl td:first-child{font-family:"Space Mono",monospace;white-space:nowrap;font-weight:700}
.tbl.recs td:first-child{color:var(--accent)}
/* achado */
.finding{background:var(--surface);border:1px solid var(--line);border-left:4px solid var(--c);
  border-radius:0 12px 12px 0;padding:16px 18px;margin:1.1rem 0}
.finding .fh{display:flex;gap:10px;align-items:baseline;flex-wrap:wrap;margin-bottom:.5rem}
.finding .fh .id{font-family:"Space Grotesk",sans-serif;font-weight:700;font-size:1.02rem}
.refs{margin:.1rem 0 .7rem;display:flex;flex-wrap:wrap;gap:6px}
.ref{background:var(--code-bg);border:1px solid var(--line);border-radius:5px;
  padding:.12em .5em;font-size:.74rem;color:var(--muted)}
.finding pre{margin:.6rem 0}
.lbl{font-family:"Space Mono",monospace;font-size:.7rem;letter-spacing:.06em;
  text-transform:uppercase;font-weight:700;color:var(--muted)}
.finding p{font-size:.92rem}
.acc{list-style:none;padding:0;margin:.4rem 0 0;font-size:.9rem}
.acc li{padding-left:1.5rem;position:relative;margin:.2rem 0}
.acc li::before{content:"";position:absolute;left:0;top:.35em;width:.8em;height:.8em;
  border:1.5px solid var(--muted);border-radius:3px}
pre{background:var(--code-bg);border:1px solid var(--line);border-radius:8px;
  padding:12px 14px;overflow-x:auto;font-family:"Space Mono","SF Mono",Menlo,monospace;
  font-size:.78rem;line-height:1.5;white-space:pre-wrap;word-break:break-word}
/* issues */
.issue{position:relative;margin:1rem 0}
.issue pre{background:var(--surface)}
.issue button{position:absolute;top:10px;right:10px;font-family:"Space Mono",monospace;
  font-size:.68rem;letter-spacing:.06em;text-transform:uppercase;padding:.3em .8em;
  border:1px solid var(--line);border-radius:6px;background:var(--paper);color:var(--ink);
  cursor:pointer}
.issue button:hover{border-color:var(--accent);color:var(--accent)}
.toc{background:var(--surface);border:1px solid var(--line);border-radius:12px;
  padding:14px 18px;margin:1.6rem 0;font-size:.9rem}
.toc ol{margin:.4rem 0;padding-left:1.4rem;columns:2;column-gap:2rem}
.toc a{color:var(--ink);text-decoration:none}
.toc a:hover{color:var(--accent)}
.foot{margin-top:3rem;padding-top:1rem;border-top:1px solid var(--line);
  font-size:.8rem;color:var(--muted)}
"""


def build_artifact_html() -> None:
    """Versão conteúdo-apenas para publicar como Artifact (o host injeta
    <!doctype>/<head>/<body>). Mesmos dados do PDF/HTML."""
    donut_path, bar_path = build_charts()
    donut_b64 = _b64_png(donut_path)
    bar_b64 = _b64_png(bar_path)

    counts = {s: 0 for s in SEV_ORDER}
    for f in FINDINGS:
        counts[f["sev"]] += 1
    total = len(FINDINGS)
    sev_var = {
        "CRÍTICA": "var(--sev-critica)", "ALTA": "var(--sev-alta)",
        "MÉDIA": "var(--sev-media)", "BAIXA": "var(--sev-baixa)",
        "INFORMATIVA": "var(--sev-info)",
    }

    def chip(sev: str) -> str:
        return f'<span class="chip" style="background:{sev_var[sev]}">{sev}</span>'

    P: list[str] = []
    P.append("<title>Auditoria de Segurança Screen Share</title>")
    P.append(
        '<link rel="stylesheet" '
        'href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@500;700'
        '&family=Space+Mono:wght@400;700&family=Libre+Franklin:wght@400;500;600&display=swap">'
    )
    P.append(f"<style>{ARTIFACT_CSS}</style>")
    P.append("<div class='page'>")

    # Masthead
    P.append("<header class='masthead'>")
    P.append("<div class='eyebrow'>Relatório de auditoria de segurança</div>")
    P.append("<h1>Screen Share</h1>")
    P.append("<div class='rule'></div>")
    P.append("<dl class='meta col'>")
    P.append(f"<dt>Data</dt><dd>{AUDIT_DATE}</dd>")
    P.append(
        "<dt>Escopo</dt><dd><code>crates/protocol</code>, <code>crates/signaling</code> "
        "(relay Axum/WebSocket + registry em memória + TURN), <code>apps/web</code> "
        "(Leptos SSR + hydrate/WASM), <code>desktop/</code> (Electron) e artefatos de "
        "implantação (<code>Dockerfile</code>, <code>docker-entrypoint.sh</code>, "
        "<code>fly.toml</code>, <code>.github/workflows</code>).</dd>"
    )
    P.append(
        "<dt>Método</dt><dd>Revisão de código estática, manual, arquivo por arquivo; "
        "histórico git varrido por segredos. Sem teste dinâmico contra ambiente vivo.</dd>"
    )
    P.append("</dl></header>")

    P.append(
        "<p class='note col'><b>Nota metodológica.</b> As cinco categorias do roteiro foram "
        "mapeadas para a stack: (1) <i>isolamento de inquilino</i> &mdash; sem banco/ORM; o "
        "mecanismo &eacute; o <i>registry</i> em mem&oacute;ria chaveado por c&oacute;digo de "
        "sala, com <code>room_code</code> + <code>peer_id</code> vinculados &agrave; "
        "conex&atilde;o no servidor. (2) <i>Permiss&atilde;o no navegador</i> &mdash; modelo de "
        "sala plano (sem papel de admin/host), ent&atilde;o quase n&atilde;o h&aacute; "
        "opera&ccedil;&atilde;o privilegiada a verificar; a &uacute;nica checagem que importa "
        "(senha) &eacute; feita no servidor. (3) <i>IDOR</i> &mdash; IDs de objeto s&atilde;o o "
        "c&oacute;digo de sala e os <code>peer_id</code> na sinaliza&ccedil;&atilde;o. "
        "(4) <i>Segredos</i> &mdash; Dockerfile, entrypoint, <code>fly.toml</code>, CI, bundle "
        "do frontend e hist&oacute;rico git. (5) <i>Entrada n&atilde;o tratada</i> &mdash; no "
        "frontend, Leptos escapa por padr&atilde;o e n&atilde;o h&aacute; "
        "<code>inner_html</code>/<code>eval</code>; foco em CSS/atributos, <i>spoofing</i> por "
        "Unicode e inje&ccedil;&atilde;o de comando no desktop.</p>"
    )

    # TOC
    P.append(
        "<nav class='toc'><b class='eyebrow'>Conte&uacute;do</b><ol>"
        "<li><a href='#resumo'>Resumo executivo</a></li>"
        "<li><a href='#fortes'>Pontos fortes</a></li>"
        "<li><a href='#fracos'>Pontos fracos</a></li>"
        "<li><a href='#detalhes'>Achados detalhados</a></li>"
        "<li><a href='#recs'>Recomenda&ccedil;&otilde;es priorizadas</a></li>"
        "<li><a href='#issues'>Issues para o GitHub</a></li>"
        "</ol></nav>"
    )

    # 1. Resumo
    P.append("<h2 id='resumo'><span class='num'>01</span>Resumo executivo</h2>")
    P.append("<div class='tally'>")
    for s in SEV_ORDER:
        P.append(
            f"<div class='tile' style='--c:{sev_var[s]}'><b>{counts[s]}</b>"
            f"<span>{s}</span></div>"
        )
    P.append(
        f"<div class='tile' style='--c:var(--sev-forte)'><b>{len(STRENGTHS)}</b>"
        f"<span>Pontos fortes</span></div></div>"
    )
    P.append(
        f"<p class='col'>A auditoria registrou <b>{total} achados verificados</b> mais "
        f"<b>{len(STRENGTHS)} pontos fortes</b> comprovados.</p>"
        "<p class='col'>O risco central n&atilde;o est&aacute; no v&iacute;deo P2P (protegido "
        "por DTLS-SRTP) e sim na <b>infraestrutura de relay TURN</b> (SSRF a metadados de "
        "nuvem e abuso de banda &mdash; F01), na <b>aus&ecirc;ncia de limites de recurso</b> "
        "no relay de sinaliza&ccedil;&atilde;o (DoS remoto trivial e n&atilde;o autenticado "
        "&mdash; F02/F03) e na <b>cadeia de suprimento do app desktop</b> (auto-update sem "
        "verifica&ccedil;&atilde;o de assinatura &mdash; F04). Nenhum achado &eacute; uma "
        "viola&ccedil;&atilde;o direta de dados/RCE remota sem pr&eacute;-condi&ccedil;&atilde;o "
        "&mdash; n&atilde;o h&aacute; banco de dados nem fun&ccedil;&otilde;es de servidor "
        "al&eacute;m de <code>/ws</code> e <code>/api/rooms/:code</code>.</p>"
    )
    P.append(
        "<div class='figrow'>"
        f"<figure class='figure'><img alt='Achados por severidade' src='{donut_b64}'>"
        "<figcaption>Achados por severidade</figcaption></figure>"
        f"<figure class='figure'><img alt='Achados por categoria' src='{bar_b64}'>"
        "<figcaption>Achados por categoria</figcaption></figure>"
        "</div>"
    )

    # 2. Fortes
    P.append("<h2 id='fortes'><span class='num'>02</span>Pontos fortes</h2>")
    for sgood in STRENGTHS:
        P.append(f"<h3>&#10003;&nbsp; {_md_inline(sgood['title'])}</h3>")
        P.append(f"<p class='muted col' style='font-size:.88rem'>{_md_inline(sgood['ev'])}</p>")

    # 3. Fracos
    P.append("<h2 id='fracos'><span class='num'>03</span>Pontos fracos (riscos centrais)</h2>")
    P.append("<table class='tbl'><tr><th>ID</th><th>Sev.</th><th>Risco</th></tr>")
    for f in FINDINGS:
        if f["sev"] in ("CRÍTICA", "ALTA", "MÉDIA"):
            P.append(
                f"<tr><td>{f['id']}</td><td>{chip(f['sev'])}</td>"
                f"<td>{_md_inline(f['title'])}</td></tr>"
            )
    P.append("</table>")

    # 4. Detalhes
    P.append("<h2 id='detalhes'><span class='num'>04</span>Achados detalhados por categoria</h2>")
    by_cat: dict[str, list] = {}
    for f in FINDINGS:
        by_cat.setdefault(f["cat"], []).append(f)
    sev_rank = {s: i for i, s in enumerate(SEV_ORDER)}
    for cat in CATEGORIES:
        if cat not in by_cat:
            continue
        P.append(f"<h3 style='font-family:\"Space Mono\",monospace;font-size:.8rem;"
                 f"letter-spacing:.08em;text-transform:uppercase;color:var(--muted);"
                 f"margin-top:1.8rem'>{_html.escape(cat)}</h3>")
        for f in sorted(by_cat[cat], key=lambda x: sev_rank[x["sev"]]):
            P.append(f"<article class='finding' style='--c:{sev_var[f['sev']]}'>")
            P.append(
                f"<div class='fh'>{chip(f['sev'])}"
                f"<span class='id'>{f['id']} &mdash; {_md_inline(f['title'])}</span></div>"
            )
            P.append("<div class='refs'>"
                     + "".join(f"<span class='ref'>{_html.escape(x)}</span>" for x in f["loc"])
                     + "</div>")
            P.append(f"<pre>{_html.escape(f['code'])}</pre>")
            P.append(f"<p><span class='lbl'>Por que &eacute; explor&aacute;vel &middot; </span>"
                     f"{_md_inline(f['why'])}</p>")
            P.append(f"<p><span class='lbl'>Impacto &middot; </span>{_md_inline(f['impact'])}</p>")
            P.append(f"<p><span class='lbl'>Corre&ccedil;&atilde;o sugerida &middot; </span>"
                     f"{_md_inline(f['fix'])}</p>")
            P.append("<p class='lbl'>Crit&eacute;rios de aceite</p><ul class='acc'>")
            for a in f["accept"]:
                P.append(f"<li>{_md_inline(a)}</li>")
            P.append("</ul></article>")

    # 5. Recs
    P.append("<h2 id='recs'><span class='num'>05</span>Recomenda&ccedil;&otilde;es priorizadas</h2>")
    P.append("<table class='tbl recs'><tr><th>Prio.</th><th>A&ccedil;&atilde;o</th></tr>")
    for r in RECOMMENDATIONS:
        P.append(f"<tr><td>{r['p']}</td><td>{_md_inline(r['text'])}</td></tr>")
    P.append("</table>")

    # 6. Issues
    P.append("<h2 id='issues'><span class='num'>06</span>Issues para o GitHub</h2>")
    P.append(
        "<p class='col'>Cada bloco &eacute; o texto completo de uma issue em Markdown, pronto "
        "para copiar e colar. Achados triviais relacionados foram agrupados numa issue "
        "&uacute;nica.</p>"
    )
    for num, title, labels, bodymd in GH_ISSUES:
        block = (
            f"--- ISSUE {num} ---\n\n"
            f"Título: {title}\n\n"
            f"Labels sugeridas: {labels}\n\n"
            f"{bodymd.strip()}\n\n"
            f"--- FIM ISSUE {num} ---"
        )
        P.append(
            "<div class='issue'>"
            "<button type='button' onclick=\"navigator.clipboard.writeText("
            "this.nextElementSibling.textContent).then(()=>{var b=this;b.textContent='copiado';"
            "setTimeout(function(){b.textContent='copiar'},1500)})\">copiar</button>"
            f"<pre>{_html.escape(block)}</pre></div>"
        )

    P.append("<p class='foot'>Gerado por <code>docs/security-audit/generate_report.py</code> "
             "&mdash; mesma fonte do PDF e do HTML completo.</p>")
    P.append("</div>")

    with open(OUT_ARTIFACT, "w", encoding="utf-8") as fh:
        fh.write("".join(P))


if __name__ == "__main__":
    import sys

    what = sys.argv[1] if len(sys.argv) > 1 else "all"
    if what in ("all", "pdf"):
        build_pdf()
        print(f"PDF gerado:      {OUT_PDF}")
    if what in ("all", "html"):
        build_html()
        print(f"HTML gerado:     {OUT_HTML}")
    if what in ("all", "artifact"):
        build_artifact_html()
        print(f"Artefato gerado: {OUT_ARTIFACT}")
