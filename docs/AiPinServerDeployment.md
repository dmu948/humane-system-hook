# AI Pin Server Deployment

`com.penumbraos.server` is a privileged/system-UID APK. Its manifest declares
`android:sharedUserId="android.uid.system"` so the app runs as uid 1000 after it
is installed by the privileged PenumbraOS system injector.

Do not install `server-release.apk` with plain `pm install -r -d`. A normal
package-manager install places the APK under `/data/app` while preserving the
system shared UID. On the AI Pin this causes zygote to abort before app code
runs because SELinux cannot assign an app process context for
`com.penumbraos.server` as uid 1000 from that install mode.

The expected failure mode for the unsafe install is:

```text
SELinux: seapp_context_lookup: No match for app with uid 1000, seinfo default, name com.penumbraos.server
zygote64: JNI FatalError called: selinux_android_setcontext(1000, ...)
```

## Build

From the repo root on the Mac:

```sh
JAVA_HOME=/opt/homebrew/Cellar/openjdk@17/17.0.19/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME=/Users/dougu/Library/Android/sdk \
ANDROID_SDK_ROOT=/Users/dougu/Library/Android/sdk \
PATH=/opt/homebrew/Cellar/openjdk@17/17.0.19/bin:/Users/dougu/.cargo/bin:$PATH \
./gradlew :server:assembleRelease
```

The APK is written to:

```text
server/build/outputs/apk/release/server-release.apk
```

## Serve From Mac

```sh
cd /Users/dougu/penumbraos-src/humane-system-hook/server/build/outputs/apk/release
python3 -m http.server 9000
```

Find the Mac IP address:

```sh
ipconfig getifaddr en0
```

## Install Or Repair From AI Pin Shell

Use this flow when the server is not running yet, or when repairing a broken
server install. It uses the privileged system injector content provider directly.
Replace `<MAC_IP>` with the Mac IP address.

```sh
cd /data/local/tmp
curl -L -o server-release.apk http://<MAC_IP>:9000/server-release.apk

content write \
  --uri content://com.penumbraos.systeminjector.staging/server-release.apk \
  < /data/local/tmp/server-release.apk

content call \
  --uri content://com.penumbraos.systeminjector.staging \
  --method install \
  --arg server-release.apk
```

If a prior normal `/data/app` install exists, uninstall it before using the
system injector:

```sh
pm uninstall com.penumbraos.server
```

Then run the `content write` and `content call` commands above.

## Start And Verify

The receiver keeps the normal boot behavior and also supports an explicit local
start action for shell repair:

```sh
am broadcast \
  -n com.penumbraos.server/.BootReceiver \
  -a com.penumbraos.server.action.START_SERVER
```

Verify that the package is no longer the unsafe `/data/app` system-UID install:

```sh
pm path com.penumbraos.server
dumpsys package com.penumbraos.server | grep -i "codePath\|sharedUser\|userId"
```

Verify the local-only API:

```sh
curl -v http://127.0.0.1:8080/api/settings
```

The Center settings API remains loopback-only on the Pin. Do not expose
`/api/settings` over LAN as part of deployment.

## Updating After Server Is Healthy

When the server is already healthy and `dev.apk_install_enabled = true`, the
repo helper can upload APKs through the running server:

```sh
node scripts/dev-deploy.mjs --host <PIN_IP_OR_URL> server
```

That helper ultimately stages APKs through
`content://com.penumbraos.systeminjector.staging`. It is not a bootstrap path:
it requires the server's HTTP API to already be listening.
