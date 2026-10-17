#!/bin/sh
# Recharge les règles udev pour que la règle PatchPad s'applique sans redémarrage.
if command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload-rules || true
  udevadm trigger --subsystem-match=hidraw || true
fi
exit 0
