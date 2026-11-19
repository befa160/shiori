# Network and HTTPS

## On your local network

Shiori only listens to the device it runs on. To reach it from the tablet, make it listen on the network **and** tell it which name or address you open it with (otherwise it refuses the page, for safety):

- standalone program: `shiori serve --listen 0.0.0.0:7878 --allowed-host 192.168.1.10` (the computer's address);
- Docker: in `compose.yaml`, `"7878:7878"` instead of `"127.0.0.1:7878:7878"`, and `SHIORI_ALLOWED_HOSTS: localhost,192.168.1.10`.

Then open `http://192.168.1.10:7878` on the tablet and sign in.

::: warning
Over `http://`, the password travels in clear text on the network: keep this for a trusted network. The installable app and offline reading need HTTPS.
:::

## From the Internet

Put a **reverse proxy** in front of Shiori: it gives an HTTPS address (for example `https://shiori.example.com`), so that passwords and sessions never travel in clear text. Example with [Caddy](https://caddyserver.com), which gets the certificate on its own:

```txt
shiori.example.com {
    reverse_proxy 127.0.0.1:7878
}
```

And tell Shiori that name: `--allowed-host shiori.example.com` (or `SHIORI_ALLOWED_HOSTS: localhost,shiori.example.com` with Docker). Nginx Proxy Manager, Traefik or Cloudflare Tunnel work too. Use a **subdomain**: Shiori does not work under a subpath (`example.com/shiori`).

Nginx example:

```nginx
server {
    server_name shiori.example.com;
    location / {
        proxy_pass http://127.0.0.1:7878;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Open to the Internet, Shiori protects itself: sign-in required, a pause that grows after several wrong passwords (per account and per address), a session cookie limited to HTTPS behind the proxy (`X-Forwarded-Proto`, `X-Forwarded-For`). Turn on **two-factor authentication** for administrator accounts. Extra protection in front of Shiori (proxy password, Authelia, Cloudflare Access) is still possible. Details: [security](/en/technical/security).

With Cloudflare (free plan): the 100 MB limit applies to each request, and Shiori only sends and receives pages of a few hundred KB, downloaded volumes included.

## The same address at home and away

At home, there is no need to go through the Internet: point the same name to the local reverse proxy with a **local DNS** (your router's setting, Pi-hole, AdGuard Home: `shiori.example.com` → the reverse proxy's local address). The reverse proxy needs a valid certificate for that name (Caddy or Let's Encrypt with a DNS challenge). You keep a single address, so a single installed app and the same downloaded volumes, and at home the traffic never leaves.

A web app cannot switch from one address to another on its own: the address is its identity (installation, downloaded volumes, local settings), and an HTTPS page cannot call an `http://192.168…` address.
