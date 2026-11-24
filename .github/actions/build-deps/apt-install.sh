#!/usr/bin/env bash
# Installs the given packages with apt-get, fast and bounded (see the comment in action.yml):
#   1. seeds /var/cache/apt/archives from $APT_DEBS (the restored actions/cache),
#   2. ranks the mirrors in /etc/apt/apt-mirrors.txt by a short throughput probe,
#   3. runs `apt-get update` + `apt-get install` with retries and per-attempt time limits,
#   4. copies exactly the installed versions' .debs back to $APT_DEBS for actions/cache/save.
# What gets installed is the same as a plain `apt-get install -y --no-install-recommends`.
set -euo pipefail

: "${APT_DEBS:?APT_DEBS must name the .deb cache directory}"
archives=/var/cache/apt/archives
mirrors=/etc/apt/apt-mirrors.txt
attempts=2
update_limit=${APT_UPDATE_LIMIT:-240}   # seconds per `apt-get update`
install_limit=${APT_INSTALL_LIMIT:-600} # seconds per `apt-get install`

mkdir -p "$APT_DEBS"
cached=$(find "$APT_DEBS" -maxdepth 1 -name '*.deb' | wc -l)
echo "Restored $cached cached .debs ($(du -sh "$APT_DEBS" | cut -f1))"
if [ "$cached" -gt 0 ]; then
  sudo cp "$APT_DEBS"/*.deb "$archives/"
fi

# Keep downloaded packages, retry failed fetches, give up on stalled connections, skip the
# Translation-* indexes (they do not change what is installed).
sudo tee /etc/apt/apt.conf.d/99annota-ci >/dev/null <<'CONF'
APT::Keep-Downloaded-Packages "true";
Binary::apt::APT::Keep-Downloaded-Packages "true";
Acquire::Retries "5";
Acquire::http::Timeout "30";
Acquire::https::Timeout "30";
Acquire::Languages "none";
CONF
sudo rm -f /etc/apt/apt.conf.d/docker-clean # only present in containers; it deletes the .debs

# Ranks every candidate mirror by the throughput of an 8-second download of a ~1.5 MB index and
# rewrites the mirror list in that order (mirror+file format: `URL<TAB>priority:N`, lower first).
# apt switches mirrors only when one fails, not when it is slow, hence the probe.
rank_mirrors() {
  [ -f "$mirrors" ] || { echo "No $mirrors; keeping the sources as they are"; return 0; }
  [ -f "$mirrors.orig" ] || sudo cp "$mirrors" "$mirrors.orig"
  echo "Original $mirrors:"; sed 's/^/  /' "$mirrors.orig"
  local codename probe dir url
  codename=$(. /etc/os-release && echo "$VERSION_CODENAME")
  probe="dists/$codename-updates/main/binary-amd64/Packages.xz"
  dir=$(mktemp -d)
  {
    cut -f1 "$mirrors.orig"
    echo http://azure.archive.ubuntu.com/ubuntu/
    echo http://archive.ubuntu.com/ubuntu/
    echo http://mirrors.edge.kernel.org/ubuntu/
  } | sed '/^\s*$/d; /^#/d; s#/*$#/#' | awk '!seen[$0]++' > "$dir/candidates"
  local i=0
  while read -r url; do
    i=$((i + 1))
    ( speed=$(curl -s -o /dev/null --connect-timeout 3 --max-time 8 -w '%{speed_download}' \
        "$url$probe" || true)
      echo "${speed:-0} $url" > "$dir/$i" ) &
  done < "$dir/candidates"
  wait
  cat "$dir"/[0-9]* | sort -k1,1 -g -r > "$dir/ranked"
  echo "Mirror throughput (bytes/s):"; sed 's/^/  /' "$dir/ranked"
  if ! awk '$1 > 0 { ok = 1 } END { exit !ok }' "$dir/ranked"; then
    echo "No mirror answered the probe; keeping $mirrors as it was"
    rm -rf "$dir"; return 0
  fi
  awk '{ printf "%s\tpriority:%d\n", $2, NR }' "$dir/ranked" | sudo tee "$mirrors" >/dev/null
  rm -rf "$dir"
}

ok=
for attempt in $(seq 1 "$attempts"); do
  echo "::group::apt attempt $attempt/$attempts"
  # A time limit that hit dpkg mid-install leaves packages half configured.
  [ "$attempt" -gt 1 ] && { sudo dpkg --configure -a || true; }
  rank_mirrors
  if sudo timeout "$update_limit" apt-get update &&
     sudo timeout "$install_limit" apt-get install -y --no-install-recommends "$@"; then
    ok=1
  fi
  echo "::endgroup::"
  [ -n "$ok" ] && break
  echo "::warning::apt attempt $attempt failed or exceeded its time limit (update ${update_limit}s, install ${install_limit}s)"
done
if [ -z "$ok" ]; then
  echo "::error::Installing the build dependencies failed $attempts times; the apt mirrors are probably too slow. Re-run the job."
  exit 1
fi

# Keep only the .debs of the versions now installed, so superseded versions do not pile up
# in the cache week after week. apt names them PACKAGE_VERSION_ARCH.deb, with ':' as %3a.
rm -f "$APT_DEBS"/*.deb
kept=0
while read -r name; do
  if [ -f "$archives/$name" ]; then cp "$archives/$name" "$APT_DEBS/"; kept=$((kept + 1)); fi
done < <(dpkg-query -W -f='${db:Status-Status} ${Package}_${Version}_${Architecture}.deb\n' |
         awk '$1 == "installed" { sub(/:/, "%3a", $2); print $2 }')
echo "Cached $kept .debs ($(du -sh "$APT_DEBS" | cut -f1)) for the next run"
