# Connecting OpenCycling to Strava

OpenCycling does not ship a shared Strava application. There is no hosted proxy
today, and a shared one would not scale anyway: Strava caps an application that
has not been through a paid review upgrade at a small number of authorized
athletes (currently 10), and that upgrade is not funded.

So the fully open approach is for each user to create their own free Strava
application and run the auth proxy themselves. Each app has its own independent
quota, so there is no shared cap to run into. This also keeps your client secret
on your own machine and never routes your data through a third party. The setup
takes about two minutes and is described below.

## 1. Create your Strava application

1. Sign in and open <https://www.strava.com/settings/api>.
2. Fill in the form (name, website, etc. can be anything).
3. Set **Authorization Callback Domain** to `localhost`. This is the key field:
   it authorizes the local redirect the desktop app listens on.
4. After creating it, note your **Client ID** and **Client Secret**.

## 2. Get the proxy

The proxy is a small standalone server that holds your client secret and performs
the token exchange. It lives in its own repository:
<https://github.com/TheElysium/opencycling-strava-proxy>.

Either download the prebuilt binary for your platform from its
[latest release](https://github.com/TheElysium/opencycling-strava-proxy/releases/latest),
or clone and build from source:

```bash
git clone https://github.com/TheElysium/opencycling-strava-proxy.git
cd opencycling-strava-proxy
```

## 3. Configure and run the proxy

The proxy reads its configuration from environment variables, falling back to a
`.env` file when a variable is not already set in the environment:

- `STRAVA_CLIENT_ID` (required)
- `STRAVA_CLIENT_SECRET` (required)
- `PROXY_PORT` (optional, defaults to `8788`)

The simplest setup is a `.env` file next to the binary (or in the cloned repo).
Copy the example and fill it in:

```bash
cp .env.example .env
```

Edit `.env`:

```
STRAVA_CLIENT_ID=<your client id>
STRAVA_CLIENT_SECRET=<your client secret>
PROXY_PORT=8788
```

Alternatively, export them as real environment variables (these take precedence
over `.env`), for example if you prefer to keep secrets out of files:

```bash
export STRAVA_CLIENT_ID=<your client id>
export STRAVA_CLIENT_SECRET=<your client secret>
```

Then run it (the downloaded binary directly, or `cargo run` from the source
checkout). It listens on `http://127.0.0.1:8788` and exposes three routes:

- `GET /config` returns your client id (used by the app to build the authorize
  URL). The secret is never exposed.
- `POST /exchange` and `POST /refresh` perform the token exchange.

Keep it running while you connect and upload.

## 4. Point the app at the proxy

In OpenCycling, open **Settings** and find the **Third-party integrations**
section. The Strava tile has an **Auth proxy URL** field that saves automatically
as you edit it. The default `http://127.0.0.1:8788` matches the proxy above, so
usually you can leave it as is. Change it only if the proxy runs on a different
port or host.

Then click **Connect** in the same tile: your browser opens the Strava
authorization page, and once you approve, the app stores your tokens.

## Notes

- **Why the URL is configurable:** the desktop app builds the authorize URL from
  the client id served by `/config`, so the app and the proxy can never use
  mismatched applications. Making the URL a setting also lets you point at a proxy
  on another machine on your LAN without rebuilding.
- **Why a shared proxy does not scale:** a single hosted proxy serves everyone
  through one Strava application, which is capped (currently 10 athletes) until a
  paid review upgrade. Running your own app sidesteps that cap entirely.
- The scopes requested are `activity:write` and `read`, enough to upload finished
  sessions as Virtual Rides.
