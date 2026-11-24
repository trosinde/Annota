# Recording elements in clean containers (task 2.3), 2026-11-24

Docker images ubuntu:24.04, ubuntu:26.04, debian:13 (universe enabled on Ubuntu); installed with `--no-install-recommends`: gstreamer1.0-tools, -pipewire, -plugins-base, -good, -ugly, -bad (second run adds gstreamer1.0-libav). Each element checked with `gst-inspect-1.0`, its plugin file mapped to its package with `dpkg -S`. Script: `check.sh` below.

## ubuntu-24.04
```
== Ubuntu 24.04.5 LTS
gstreamer1.0-pipewire:amd64 1.0.5-1ubuntu3.3
gstreamer1.0-plugins-bad:amd64 1.24.2-1ubuntu4
gstreamer1.0-plugins-base:amd64 1.24.2-1ubuntu0.7
gstreamer1.0-plugins-good:amd64 1.24.2-1ubuntu1.8
gstreamer1.0-plugins-ugly:amd64 1.24.1-1build1
gstreamer1.0-tools 1.24.2-1ubuntu0.2
pipewiresrc ok gstreamer1.0-pipewire
pulsesrc ok gstreamer1.0-plugins-good
level ok gstreamer1.0-plugins-good
audiomixer ok gstreamer1.0-plugins-base
videocrop ok gstreamer1.0-plugins-good
videorate ok gstreamer1.0-plugins-base
videoscale ok gstreamer1.0-plugins-base
videoconvert ok gstreamer1.0-plugins-base
x264enc ok gstreamer1.0-plugins-ugly
opusenc ok gstreamer1.0-plugins-base
opusdec ok gstreamer1.0-plugins-base
matroskamux ok gstreamer1.0-plugins-good
matroskademux ok gstreamer1.0-plugins-good
h264parse ok gstreamer1.0-plugins-bad
mp4mux ok gstreamer1.0-plugins-good
vp9enc ok gstreamer1.0-plugins-good
webmmux ok gstreamer1.0-plugins-good
voaacenc ok gstreamer1.0-plugins-bad
volume ok gstreamer1.0-plugins-base
audioconvert ok gstreamer1.0-plugins-base
audioresample ok gstreamer1.0-plugins-base
appsrc ok gstreamer1.0-plugins-base
appsink ok gstreamer1.0-plugins-base
identity ok libgstreamer1.0-0
queue ok libgstreamer1.0-0
```
## ubuntu-26.04
```
== Ubuntu 26.04.1 LTS
gstreamer1.0-pipewire:amd64 1.6.2-1ubuntu1.2
gstreamer1.0-plugins-bad:amd64 1.28.2-1ubuntu1.2
gstreamer1.0-plugins-base:amd64 1.28.2-1ubuntu0.3
gstreamer1.0-plugins-extra:amd64 1.28.2-1ubuntu1.2
gstreamer1.0-plugins-good:amd64 1.28.2-2ubuntu0.4
gstreamer1.0-plugins-ugly:amd64 1.28.2-1
gstreamer1.0-tools 1.28.2-1ubuntu0.1
pipewiresrc ok gstreamer1.0-pipewire
pulsesrc ok gstreamer1.0-plugins-good
level ok gstreamer1.0-plugins-good
audiomixer ok gstreamer1.0-plugins-base
videocrop ok gstreamer1.0-plugins-good
videorate ok gstreamer1.0-plugins-base
videoscale ok gstreamer1.0-plugins-base
videoconvert ok gstreamer1.0-plugins-base
x264enc ok gstreamer1.0-plugins-ugly
opusenc ok gstreamer1.0-plugins-base
opusdec ok gstreamer1.0-plugins-base
matroskamux ok gstreamer1.0-plugins-good
matroskademux ok gstreamer1.0-plugins-good
h264parse ok gstreamer1.0-plugins-bad
mp4mux ok gstreamer1.0-plugins-good
vp9enc ok gstreamer1.0-plugins-good
webmmux ok gstreamer1.0-plugins-good
voaacenc ok gstreamer1.0-plugins-bad
volume ok gstreamer1.0-plugins-base
audioconvert ok gstreamer1.0-plugins-base
audioresample ok gstreamer1.0-plugins-base
appsrc ok gstreamer1.0-plugins-base
appsink ok gstreamer1.0-plugins-base
identity ok libgstreamer1.0-0
queue ok libgstreamer1.0-0
```
## debian-13
```
== Debian GNU/Linux 13 (trixie)
gstreamer1.0-pipewire:amd64 1.4.2-1
gstreamer1.0-plugins-bad:amd64 1.26.2-3+deb13u3
gstreamer1.0-plugins-base:amd64 1.26.2-1+deb13u2
gstreamer1.0-plugins-good:amd64 1.26.2-1+deb13u2
gstreamer1.0-plugins-ugly:amd64 1.26.3-4+deb13u1
gstreamer1.0-tools 1.26.2-2
pipewiresrc ok gstreamer1.0-pipewire
pulsesrc ok gstreamer1.0-plugins-good
level ok gstreamer1.0-plugins-good
audiomixer ok gstreamer1.0-plugins-base
videocrop ok gstreamer1.0-plugins-good
videorate ok gstreamer1.0-plugins-base
videoscale ok gstreamer1.0-plugins-base
videoconvert ok gstreamer1.0-plugins-base
x264enc ok gstreamer1.0-plugins-ugly
opusenc ok gstreamer1.0-plugins-base
opusdec ok gstreamer1.0-plugins-base
matroskamux ok gstreamer1.0-plugins-good
matroskademux ok gstreamer1.0-plugins-good
h264parse ok gstreamer1.0-plugins-bad
mp4mux ok gstreamer1.0-plugins-good
vp9enc ok gstreamer1.0-plugins-good
webmmux ok gstreamer1.0-plugins-good
voaacenc ok gstreamer1.0-plugins-bad
volume ok gstreamer1.0-plugins-base
audioconvert ok gstreamer1.0-plugins-base
audioresample ok gstreamer1.0-plugins-base
appsrc ok gstreamer1.0-plugins-base
appsink ok gstreamer1.0-plugins-base
identity ok libgstreamer1.0-0
queue ok libgstreamer1.0-0
```
## Second run (decoder and timestamper)
```
all three images: h264timestamper ok gstreamer1.0-plugins-bad; openh264dec ok gstreamer1.0-plugins-bad; avdec_h264 ok gstreamer1.0-libav; vp9dec ok gstreamer1.0-plugins-good
```
## check.sh
```sh
#!/bin/sh
# runs inside a clean container: installs the planned Depends and checks every element
export DEBIAN_FRONTEND=noninteractive
. /etc/os-release
if [ "$ID" = ubuntu ]; then sed -i 's/^Components: main restricted$/Components: main restricted universe/; s/Components: main$/Components: main universe/' /etc/apt/sources.list.d/ubuntu.sources 2>/dev/null; fi
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends gstreamer1.0-tools gstreamer1.0-pipewire gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly gstreamer1.0-plugins-bad >/dev/null 2>&1 || echo "APT FAILED"
echo "== $PRETTY_NAME"
dpkg -l 'gstreamer1.0-*' | awk '/^ii/{print $2, $3}'
for e in pipewiresrc pulsesrc level audiomixer videocrop videorate videoscale videoconvert x264enc opusenc opusdec matroskamux matroskademux h264parse mp4mux vp9enc webmmux voaacenc volume audioconvert audioresample appsrc appsink identity queue; do
  f=$(gst-inspect-1.0 $e 2>/dev/null | awk '/Filename/{print $2}')
  if [ -n "$f" ]; then echo "$e ok $(dpkg -S $f 2>/dev/null | cut -d: -f1)"; else echo "$e MISSING"; fi
done
```
