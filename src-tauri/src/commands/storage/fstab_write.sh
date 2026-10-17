# Fixed privileged writer: caller supplies the expected SHA-256, then new text.
# No request value is evaluated as shell code or used as a destination path.
set -eu
umask 077
fstab='/etc/fstab'
[ -f "$fstab" ] && [ ! -L "$fstab" ] || { echo 'fstab não é um arquivo regular.' >&2; exit 1; }
exec 9>"$fstab.peligames.lock"
/usr/bin/flock -x 9
IFS= read -r expected
IFS= read -r replacement
[ "${#expected}" = 64 ] || exit 1
[ "${#replacement}" = 64 ] || exit 1
case "$expected" in *[!a-f0-9]*) exit 1;; esac
case "$replacement" in *[!a-f0-9]*) exit 1;; esac
current=$(/usr/bin/sha256sum -- "$fstab"); current=${current%% *}
[ "$current" = "$expected" ] || { echo 'fstab mudou; atualize a lista e tente novamente.' >&2; exit 20; }
directory=$(/usr/bin/dirname -- "$fstab")
temporary=$(/usr/bin/mktemp "$directory/.fstab.peligames.XXXXXX")
backup=$(/usr/bin/mktemp "$directory/.fstab.peligames-backup.XXXXXX")
trap '/usr/bin/rm -f -- "$temporary" "$backup"' EXIT
/usr/bin/cat > "$temporary"
received=$(/usr/bin/sha256sum -- "$temporary"); received=${received%% *}
[ "$received" = "$replacement" ] || { echo 'Pedido incompleto; fstab preservado.' >&2; exit 21; }
/usr/bin/cp --attributes-only --preserve=all -- "$fstab" "$temporary"
/usr/bin/cp --preserve=all -- "$fstab" "$backup"
/usr/bin/sync -f "$temporary"
/usr/bin/sync -f "$backup"
current=$(/usr/bin/sha256sum -- "$fstab"); current=${current%% *}
[ "$current" = "$expected" ] || { echo 'fstab mudou; atualize a lista e tente novamente.' >&2; exit 20; }
[ ! -L "$fstab" ] || exit 1
/usr/bin/mv -f -- "$backup" "$fstab.peligames.bak"
/usr/bin/mv -f -- "$temporary" "$fstab"
/usr/bin/sync -f "$directory"
if [ "${1:-write}" = reload ]; then
  /usr/bin/systemctl daemon-reload || { echo 'Entrada removida, mas não foi possível recarregar o systemd.' >&2; exit 1; }
fi
