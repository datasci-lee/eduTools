#!/bin/bash
# 디자이너가 내보낸 eduTools-symbols 폴더를 저장소 배치로 옮긴다(다시 내보낼 때도 같은 명령).
#   usage: assets/import-symbols.sh <eduTools-symbols 폴더>
#   svg/ 중 symbol-map.tsv 에 있는 것 → assets/icons/<키>.svg (앱이 쓴다)
#   나머지 svg/·attr/·windows/·README.md → assets/symbols/ (원본 이름·구조 그대로)
set -euo pipefail
SRC=${1:?usage: import-symbols.sh <eduTools-symbols 폴더>}
HERE=$(cd "$(dirname "$0")" && pwd)
[ -d "$SRC/svg" ] || { echo "$SRC/svg 가 없습니다" >&2; exit 64; }
mkdir -p "$HERE/icons" "$HERE/symbols/svg"
mapped=$(mktemp); trap 'rm -f "$mapped"' EXIT
while IFS=$'\t' read -r design key; do
    case "$design" in ''|\#*) continue;; esac
    [ -f "$SRC/svg/$design.svg" ] || { echo "경고: 디자인 파일 없음 — $design.svg (키 $key)" >&2; continue; }
    cp "$SRC/svg/$design.svg" "$HERE/icons/$key.svg"
    echo "$design" >> "$mapped"
done < "$HERE/symbol-map.tsv"
n_other=0
for f in "$SRC"/svg/*.svg; do
    name=$(basename "$f" .svg)
    grep -qxF "$name" "$mapped" && continue
    cp "$f" "$HERE/symbols/svg/$name.svg"; n_other=$((n_other + 1))
done
for d in attr windows; do
    [ -d "$SRC/$d" ] && { mkdir -p "$HERE/symbols/$d"; cp -R "$SRC/$d/." "$HERE/symbols/$d/"; }
done
[ -f "$SRC/README.md" ] && cp "$SRC/README.md" "$HERE/symbols/README.md"
find "$HERE/symbols" -name .DS_Store -delete
echo "앱 아이콘 $(wc -l < "$mapped" | tr -d ' ')개 → assets/icons/, 보관 ${n_other}개 → assets/symbols/svg/"
