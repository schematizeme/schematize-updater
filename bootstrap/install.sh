#!/usr/bin/env bash
# Bootstrap do schematize-updater (Linux/macOS) — baixa o binário do updater e roda `install`.
# É a ÚNICA coisa que o usuário roda à mão; daí em diante o updater cuida de tudo (app CLI+GUI,
# versão, toolchain quando precisar compilar). Uso:
#   curl -fsSL https://raw.githubusercontent.com/schematizeme/schematize-updater/main/bootstrap/install.sh | bash
set -euo pipefail

REPO="schematizeme/schematize-updater"
DEST="${SCHEMATIZE_UPDATER_DIR:-$HOME/.local/bin}"

os="$(uname -s)"; arch="$(uname -m)"
case "$os/$arch" in
  Linux/x86_64)          asset="schematize-updater-linux-x86_64" ;;
  Darwin/arm64)          asset="schematize-updater-macos-arm64" ;;
  Darwin/x86_64)         asset="schematize-updater-macos-x86_64" ;;
  *) echo "plataforma não suportada pelo bootstrap: $os/$arch — compile o updater do fonte (cargo build --release)"; exit 1 ;;
esac

url="https://github.com/$REPO/releases/latest/download/$asset"
mkdir -p "$DEST"
bin="$DEST/schematize-updater"
echo "→ baixando o schematize-updater ($asset)…"
if ! curl -fsSL -o "$bin" "$url"; then
  echo "não achei o binário pré-compilado do updater ($url)."
  echo "Se você tem Rust: git clone https://github.com/$REPO && cd schematize-updater && cargo install --path ."
  exit 1
fi
chmod +x "$bin"

# Garante ~/.local/bin no PATH (idempotente).
case ":$PATH:" in *":$DEST:"*) : ;; *)
  for rc in "$HOME/.bashrc" "$HOME/.profile" "$HOME/.zshrc"; do
    [ -e "$rc" ] || continue
    grep -q '.local/bin' "$rc" 2>/dev/null || printf '\n# schematize-updater\nexport PATH="%s:$PATH"\n' "$DEST" >> "$rc"
  done ;;
esac

echo "→ instalando o app schematize (binário pronto se houver, senão compila do fonte)…"
exec "$bin" install
